//! Pure helpers for ProjectPickerPopover / ProjectSelectPopover / EntryEditDialog.
use crate::api::Project;

/// Case-insensitive match on project or client name, grouped by client name in first-seen order.
pub fn filter_projects<'a>(projects: &'a [Project], query: &str) -> Vec<(String, Vec<&'a Project>)> {
    let q = query.to_lowercase();
    let mut groups: Vec<(String, Vec<&Project>)> = Vec::new();
    for p in projects.iter().filter(|p| !p.archived) {
        if !q.is_empty() && !p.name.to_lowercase().contains(&q) && !p.client.name.to_lowercase().contains(&q) {
            continue;
        }
        match groups.iter_mut().find(|(c, _)| *c == p.client.name) {
            Some((_, list)) => list.push(p),
            None => groups.push((p.client.name.clone(), vec![p])),
        }
    }
    groups
}

/// Prefix matches (case-insensitive, exact match excluded), at most 8; all titles when empty.
pub fn title_suggestions(titles: &[String], input: &str) -> Vec<String> {
    let q = input.trim().to_lowercase();
    titles
        .iter()
        .filter(|t| q.is_empty() || (t.to_lowercase().starts_with(&q) && t.to_lowercase() != q))
        .take(8)
        .cloned()
        .collect()
}

/// `new Set([...locked, ...selected])`
pub fn merged_tag_ids(selected: &[String], locked: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in locked.iter().chain(selected) {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

pub fn locked_tag_ids(project: Option<&Project>) -> Vec<String> {
    project.map(|p| p.tags.iter().map(|t| t.tag_id.clone()).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fake_data;

    #[test]
    fn filter_projects_matches_client_or_project_name() {
        let ps = fake_data().projects;
        let all = filter_projects(&ps, "acme");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0, "Acme");
        assert_eq!(all[0].1.len(), 2);
        let beta = filter_projects(&ps, "BET");
        assert_eq!(beta[0].1.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["Beta"]);
        assert!(filter_projects(&ps, "zzz").is_empty());
    }

    #[test]
    fn title_suggestions_prefix_limit_8() {
        let mut titles: Vec<String> = (0..10).map(|i| format!("定例{i}")).collect();
        titles.push("定例".into());
        titles.push("別件".into());
        let s = title_suggestions(&titles, " 定例 ");
        assert_eq!(s.len(), 8);
        assert!(!s.contains(&"定例".to_string()));
        assert_eq!(title_suggestions(&titles, "").len(), 8);
        assert_eq!(title_suggestions(&["Daily".into()], "dai"), vec!["Daily".to_string()]);
    }

    #[test]
    fn merged_tag_ids_unions_locked() {
        assert_eq!(merged_tag_ids(&["b".into(), "a".into()], &["a".into(), "c".into()]), vec!["a", "c", "b"]);
    }
}
