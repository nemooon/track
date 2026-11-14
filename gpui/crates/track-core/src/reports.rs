//! Report aggregation (src/server/routes/reports.ts).
use crate::dates::{month_range, week_range};
use crate::error::Result;
use crate::model::*;
use crate::Db;
use chrono::{DateTime, Local, Utc};
use std::collections::HashMap;

const NONE: &str = "__none__";
const NONE_COLOR: &str = "#9ca3af";

use crate::dates::local_midnight;

/// [from, to) for the query's week (Sunday start) or month, in local time.
pub fn query_range(q: &ReportQuery) -> (DateTime<Utc>, DateTime<Utc>) {
    let day = q.anchor.with_timezone(&Local).date_naive();
    let (f, t) = if q.range == ReportRange::Week { week_range(day) } else { month_range(day) };
    (local_midnight(f), local_midnight(t))
}

/// Weeks in the activity heatmap (about a year, like GitHub's).
pub const HEATMAP_WEEKS: i64 = 53;

/// The days the activity heatmap shows, `[from, to)`: `HEATMAP_WEEKS` Sunday-start weeks with the
/// week of the period's first day in the middle (26 weeks before it, 26 after).
pub fn heatmap_range(q: &ReportQuery) -> (chrono::NaiveDate, chrono::NaiveDate) {
    use crate::dates::week_start;
    use chrono::Duration;
    let first = query_range(q).0.with_timezone(&Local).date_naive();
    let centre = week_start(first);
    let before = HEATMAP_WEEKS / 2;
    (centre - Duration::weeks(before), centre + Duration::weeks(HEATMAP_WEEKS - before))
}

fn split_none(ids: &[String]) -> (Vec<&String>, bool) {
    let real: Vec<&String> = ids.iter().filter(|i| *i != NONE).collect();
    let has_none = real.len() != ids.len();
    (real, has_none)
}

fn entry_tag_ids(e: &TimeEntry) -> impl Iterator<Item = &String> {
    e.tags.iter().map(|t| &t.tag_id)
}

fn project_tag_ids(e: &TimeEntry) -> impl Iterator<Item = &String> {
    e.project.iter().flat_map(|p| p.tags.iter().map(|t| &t.tag_id))
}

/// `buildWhere`: each dimension ORs its ids, dimensions AND together.
fn matches(e: &TimeEntry, q: &ReportQuery) -> bool {
    let (clients, none) = split_none(&q.client_ids);
    if !clients.is_empty() || none {
        let ok = (none && e.project_id.is_none()) || e.project.as_ref().is_some_and(|p| clients.contains(&&p.client_id));
        if !ok {
            return false;
        }
    }
    let (projects, none) = split_none(&q.project_ids);
    if !projects.is_empty() || none {
        let ok = (none && e.project_id.is_none()) || e.project_id.as_ref().is_some_and(|p| projects.contains(&p));
        if !ok {
            return false;
        }
    }
    let (tags, none) = split_none(&q.tag_ids);
    if !tags.is_empty() || none {
        let no_tags = e.tags.is_empty() && project_tag_ids(e).next().is_none();
        let ok = (none && no_tags) || entry_tag_ids(e).chain(project_tag_ids(e)).any(|t| tags.contains(&t));
        if !ok {
            return false;
        }
    }
    true
}

/// Minutes inside [from, to) minus the prorated break.
fn visible_minutes(e: &TimeEntry, from: DateTime<Utc>, to: DateTime<Utc>) -> i64 {
    let s = e.start.max(from);
    let en = e.end.min(to);
    let vis_ms = (en - s).num_milliseconds() as f64;
    let full_ms = (e.end - e.start).num_milliseconds() as f64;
    let gross = (vis_ms / 60_000.).round() as i64;
    let brk = if full_ms > 0. { (e.break_minutes as f64 * (vis_ms / full_ms)).round() as i64 } else { 0 };
    (gross - brk).max(0)
}

impl Db {
    fn report_source(&self, q: &ReportQuery) -> Result<(DateTime<Utc>, DateTime<Utc>, Vec<TimeEntry>)> {
        let (from, to) = query_range(q);
        let entries = self.entries(from, to)?.into_iter().filter(|e| matches(e, q)).collect();
        Ok((from, to, entries))
    }

    pub fn report(&self, q: &ReportQuery) -> Result<ReportResponse> {
        let (from, to, entries) = self.report_source(q)?;
        let mut order: Vec<String> = Vec::new();
        let mut rows: HashMap<String, ReportRow> = HashMap::new();
        let mut add = |key: String, label: String, color: Option<String>, mins: i64, order: &mut Vec<String>| {
            rows.entry(key.clone())
                .and_modify(|r| r.total_minutes += mins)
                .or_insert_with(|| {
                    order.push(key.clone());
                    ReportRow { key, label, color, total_minutes: mins }
                });
        };
        let mut total = 0;
        for e in &entries {
            let mins = visible_minutes(e, from, to);
            total += mins;
            match q.group_by {
                GroupBy::Tag => {
                    let mut tags: Vec<(String, String, String)> = Vec::new();
                    for t in e.tags.iter().chain(e.project.iter().flat_map(|p| p.tags.iter())) {
                        if !tags.iter().any(|(id, ..)| *id == t.tag_id) {
                            tags.push((t.tag_id.clone(), t.tag.name.clone(), t.tag.color.clone()));
                        }
                    }
                    let include_all = q.tag_ids.is_empty();
                    let visible: Vec<_> = tags.into_iter().filter(|(id, ..)| include_all || q.tag_ids.contains(id)).collect();
                    if visible.is_empty() {
                        if include_all {
                            add(NONE.into(), "タグなし".into(), Some(NONE_COLOR.into()), mins, &mut order);
                        }
                    } else {
                        for (id, name, color) in visible {
                            add(id, name, Some(color), mins, &mut order);
                        }
                    }
                }
                _ => match (&e.project, q.group_by) {
                    (None, _) => add(NONE.into(), "プロジェクトなし".into(), Some(NONE_COLOR.into()), mins, &mut order),
                    (Some(p), GroupBy::Client) => add(p.client_id.clone(), p.client.name.clone(), None, mins, &mut order),
                    (Some(p), _) => add(p.id.clone(), format!("{} · {}", p.client.name, p.name), Some(p.color.clone()), mins, &mut order),
                },
            }
        }
        // stable sort by minutes, ties keep first-seen order (like Array.prototype.sort)
        let mut out: Vec<ReportRow> = order.into_iter().filter_map(|k| rows.remove(&k)).collect();
        out.sort_by_key(|r| std::cmp::Reverse(r.total_minutes));
        Ok(ReportResponse { rows: out, total_minutes: total, range: DateSpan { from, to } })
    }

    /// Minutes per local day over `heatmap_range`, with the query's filters (every day listed,
    /// in order; days without entries are 0).
    pub fn heatmap(&self, q: &ReportQuery) -> Result<Vec<(chrono::NaiveDate, i64)>> {
        use chrono::Duration;
        let (from, to) = heatmap_range(q);
        let mut days: Vec<(chrono::NaiveDate, i64)> = from.iter_days().take_while(|d| *d < to).map(|d| (d, 0)).collect();
        for e in self.entries(local_midnight(from), local_midnight(to))?.into_iter().filter(|e| matches(e, q)) {
            let mut d = e.start.with_timezone(&Local).date_naive().max(from);
            let last = e.end.with_timezone(&Local).date_naive();
            while d <= last && d < to {
                let mins = visible_minutes(&e, local_midnight(d), local_midnight(d + Duration::days(1)));
                if let Some(slot) = days.get_mut((d - from).num_days() as usize) {
                    slot.1 += mins;
                }
                d += Duration::days(1);
            }
        }
        Ok(days)
    }

    /// Project totals for the query's range and filters, whatever the screen groups by.
    pub fn weekly_copy(&self, q: &ReportQuery) -> Result<String> {
        let by_project = ReportQuery { group_by: GroupBy::Project, ..q.clone() };
        Ok(crate::report::view::weekly_copy_text(&self.report(&by_project)?.rows))
    }

    pub fn report_entries(&self, q: &ReportQuery) -> Result<ReportEntriesResponse> {
        let (from, to, entries) = self.report_source(q)?;
        let mut total = 0;
        let entries = entries
            .into_iter()
            .map(|e| {
                let minutes = visible_minutes(&e, from, to);
                total += minutes;
                ReportEntry {
                    minutes,
                    project: e.project.as_ref().map(|p| ReportEntryProject {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        color: p.color.clone(),
                        client: ReportEntryClient { id: p.client.id.clone(), name: p.client.name.clone() },
                    }),
                    tags: e.tags.iter().map(|t| t.tag.clone()).collect(),
                    id: e.id,
                    start: e.start,
                    end: e.end,
                    title: e.title,
                    note: e.note,
                }
            })
            .collect();
        Ok(ReportEntriesResponse { entries, total_minutes: total })
    }
}
