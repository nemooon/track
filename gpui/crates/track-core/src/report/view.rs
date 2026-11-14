//! Pure helpers behind the Reports screen (ReportsPage.tsx, ReportRow.tsx, FilterMultiSelect.tsx).
use crate::model::{GroupBy, Project, ReportQuery, ReportRow};

pub const PALETTE: [&str; 10] = ["#3b82f6", "#ef4444", "#10b981", "#f59e0b", "#8b5cf6", "#ec4899", "#06b6d4", "#84cc16", "#f97316", "#6366f1"];

#[derive(Debug, Clone, PartialEq)]
pub struct FilterOption {
    pub id: String,
    pub label: String,
    pub hint: Option<String>,
    pub color: Option<String>,
}

pub fn filter_label(label: &str, options: &[FilterOption], selected: &[String]) -> String {
    match selected {
        [] => format!("すべての{label}"),
        [one] => options.iter().find(|o| &o.id == one).map(|o| o.label.clone()).unwrap_or_else(|| format!("{label}: 1")),
        many => format!("{label}: {}件", many.len()),
    }
}

pub fn filter_options<'a>(options: &'a [FilterOption], query: &str) -> Vec<&'a FilterOption> {
    let q = query.to_lowercase();
    options
        .iter()
        .filter(|o| q.is_empty() || o.label.to_lowercase().contains(&q) || o.hint.as_ref().is_some_and(|h| h.to_lowercase().contains(&q)))
        .collect()
}

/// After the client filter changes, keep only project selections under the chosen clients.
pub fn keep_projects_for_clients(selected_projects: &[String], clients: &[String], projects: &[Project]) -> Vec<String> {
    if clients.is_empty() {
        return selected_projects.to_vec();
    }
    selected_projects
        .iter()
        .filter(|pid| projects.iter().find(|p| &p.id == *pid).is_some_and(|p| clients.contains(&p.client_id)))
        .cloned()
        .collect()
}

/// `/reports/entries` filters for a group row: the row's own dimension replaces the base ids.
pub fn child_query(base: &ReportQuery, kind: GroupBy, key: &str) -> ReportQuery {
    let mut q = base.clone();
    match kind {
        GroupBy::Client => q.client_ids = vec![key.to_string()],
        GroupBy::Project => q.project_ids = vec![key.to_string()],
        GroupBy::Tag => q.tag_ids = vec![key.to_string()],
    }
    q
}

/// Project rows are labelled "Client · Project"; the summary shows just the project.
pub fn display_label(group_by: GroupBy, label: &str) -> String {
    if group_by == GroupBy::Project { label.rsplit(" · ").next().unwrap_or(label).to_string() } else { label.to_string() }
}

pub fn group_key(g: GroupBy) -> &'static str {
    match g {
        GroupBy::Client => "client",
        GroupBy::Project => "project",
        GroupBy::Tag => "tag",
    }
}

pub fn row_path(g: GroupBy, key: &str) -> String {
    format!("{}:{key}", group_key(g))
}

pub fn row_color(color: Option<&str>, index: usize) -> String {
    color.map(str::to_string).unwrap_or_else(|| PALETTE[index % PALETTE.len()].to_string())
}

pub fn percent(part: i64, total: i64) -> i64 {
    if total > 0 { (part as f64 / total as f64 * 100.).round() as i64 } else { 0 }
}

/// Hours as a decimal for timesheets: 90 → "1.5", 80 → "1.33", 120 → "2".
/// Rounded to hundredths, trailing zeros dropped.
pub fn decimal_hours(minutes: i64) -> String {
    let sign = if minutes < 0 { "-" } else { "" };
    let hundredths = (minutes.abs() * 200 + 60) / 120;
    let (whole, frac) = (hundredths / 100, hundredths % 100);
    let frac = format!("{frac:02}");
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() { format!("{sign}{whole}") } else { format!("{sign}{whole}.{frac}") }
}

/// One `name\thours` line per project row of a project-grouped report, for pasting into a timesheet.
pub fn weekly_copy_text(rows: &[ReportRow]) -> String {
    rows.iter().map(|r| format!("{}\t{}", display_label(GroupBy::Project, &r.label), decimal_hours(r.total_minutes))).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_hours_formats() {
        assert_eq!(decimal_hours(90), "1.5");
        assert_eq!(decimal_hours(80), "1.33");
        assert_eq!(decimal_hours(120), "2");
        assert_eq!(decimal_hours(0), "0");
        assert_eq!(decimal_hours(45), "0.75");
        assert_eq!(decimal_hours(1), "0.02");
    }
    use crate::model::{GroupBy, ReportQuery, ReportRange};
    use crate::report::fixtures::projects;

    fn opts() -> Vec<FilterOption> {
        vec![
            FilterOption { id: "a".into(), label: "Alpha".into(), hint: Some("Acme".into()), color: None },
            FilterOption { id: "b".into(), label: "Beta".into(), hint: None, color: None },
        ]
    }

    #[test]
    fn filter_trigger_label() {
        assert_eq!(filter_label("プロジェクト", &opts(), &[]), "すべてのプロジェクト");
        assert_eq!(filter_label("プロジェクト", &opts(), &["b".into()]), "Beta");
        assert_eq!(filter_label("プロジェクト", &opts(), &["a".into(), "b".into()]), "プロジェクト: 2件");
    }

    #[test]
    fn filter_options_match_label_or_hint() {
        assert_eq!(filter_options(&opts(), "acm").len(), 1);
        assert_eq!(filter_options(&opts(), "BE")[0].id, "b");
        assert_eq!(filter_options(&opts(), "").len(), 2);
    }

    #[test]
    fn client_change_drops_unmatched_projects() {
        let ps = projects();
        assert_eq!(keep_projects_for_clients(&["p1".into(), "zz".into()], &["c1".into()], &ps), vec!["p1".to_string()]);
        assert!(keep_projects_for_clients(&["p1".into()], &["other".into()], &ps).is_empty());
        // clearing the client filter keeps project selections
        assert_eq!(keep_projects_for_clients(&["p1".into()], &[], &ps), vec!["p1".to_string()]);
    }

    #[test]
    fn child_query_replaces_dimension_of_the_row() {
        let base = ReportQuery {
            range: ReportRange::Week,
            anchor: "2026-11-11T15:00:00Z".parse().unwrap(),
            group_by: GroupBy::Project,
            client_ids: vec!["c1".into()],
            project_ids: vec!["p1".into(), "p2".into()],
            tag_ids: vec![],
            include_leave: false,
        };
        let q = child_query(&base, GroupBy::Project, "p2");
        assert_eq!((q.client_ids.clone(), q.project_ids.clone()), (vec!["c1".to_string()], vec!["p2".to_string()]));
        assert_eq!(child_query(&base, GroupBy::Tag, "__none__").tag_ids, vec!["__none__".to_string()]);
    }

    #[test]
    fn row_labels_and_paths() {
        assert_eq!(display_label(GroupBy::Project, "Acme · Alpha"), "Alpha");
        assert_eq!(display_label(GroupBy::Client, "Acme · X"), "Acme · X");
        assert_eq!(row_path(GroupBy::Tag, "t1"), "tag:t1");
        assert_eq!(row_color(None, 11), "#ef4444");
        assert_eq!(row_color(Some("#123456"), 0), "#123456");
        assert_eq!(percent(30, 90), 33);
        assert_eq!(percent(1, 0), 0);
    }
}
