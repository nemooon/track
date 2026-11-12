//! groupEntriesByTitle (ReportRow.tsx) for the 「まとめる」 switch.
use crate::api::{ReportEntry, ReportEntryProject, Tag};

#[derive(Debug, Clone, PartialEq)]
pub struct GroupedEntry {
    pub key: String,
    pub title: Option<String>,
    pub minutes: i64,
    pub count: usize,
    pub project: Option<ReportEntryProject>,
    pub tags: Vec<Tag>,
}

pub fn group_entries_by_title(entries: &[ReportEntry]) -> Vec<GroupedEntry> {
    let mut out: Vec<GroupedEntry> = Vec::new();
    for e in entries {
        let t = e.title.as_deref().unwrap_or("").trim();
        let key = if t.is_empty() { "__untitled__".to_string() } else { t.to_string() };
        if let Some(g) = out.iter_mut().find(|g| g.key == key) {
            g.minutes += e.minutes;
            g.count += 1;
            for tag in &e.tags {
                if !g.tags.iter().any(|x| x.id == tag.id) {
                    g.tags.push(tag.clone());
                }
            }
            if let (Some(a), Some(b)) = (&g.project, &e.project) {
                if a.id != b.id {
                    g.project = None;
                }
            }
        } else {
            out.push(GroupedEntry { key, title: e.title.clone(), minutes: e.minutes, count: 1, project: e.project.clone(), tags: e.tags.clone() });
        }
    }
    out.sort_by(|a, b| b.minutes.cmp(&a.minutes));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::report::fixtures::*;

    #[test]
    fn group_same_titles_drops_mixed_project() {
        let es = vec![
            report_entry("1", 30, Some(" 定例 "), None, Some(("p1", "A", "C"))),
            report_entry("2", 60, Some("定例"), None, Some(("p2", "B", "C"))),
            report_entry("3", 120, None, None, Some(("p1", "A", "C"))),
        ];
        let g = group_entries_by_title(&es);
        assert_eq!(g.len(), 2);
        assert_eq!((g[0].key.as_str(), g[0].minutes, g[0].count), ("__untitled__", 120, 1));
        assert_eq!((g[1].key.as_str(), g[1].minutes, g[1].count), ("定例", 90, 2));
        assert!(g[1].project.is_none());
        assert!(g[0].project.is_some());
    }
}
