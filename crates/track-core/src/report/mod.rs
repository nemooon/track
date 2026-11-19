pub mod aggregation;
pub mod cache;
pub mod copy;
pub mod group;
pub mod view;
pub mod weekly;

#[cfg(test)]
pub(crate) mod fixtures {
    use crate::model::*;

    /// Client "Acme" (c1) with projects Alpha (p1, tag t1) and Beta (p2).
    pub fn projects() -> Vec<Project> {
        let client = Client { id: "c1".into(), name: "Acme".into(), archived: false };
        let tag = Tag { id: "t1".into(), name: "定例".into(), color: "#22c55e".into() };
        vec![
            Project { id: "p1".into(), client_id: "c1".into(), name: "Alpha".into(), color: "#3b82f6".into(), archived: false, client: client.clone(), tags: vec![TagLink { tag_id: "t1".into(), tag }] },
            Project { id: "p2".into(), client_id: "c1".into(), name: "Beta".into(), color: "#ef4444".into(), archived: false, client, tags: vec![] },
        ]
    }

    pub fn col_field(id: &str, field: ReportCopyField) -> ReportCopyColumn {
        ReportCopyColumn::Field { id: id.into(), field, label: None, duration_format: None }
    }

    pub fn format(id: &str, delimiter: &str, include_header: bool, ai_prompt: &str, columns: Vec<ReportCopyColumn>) -> ReportCopyFormat {
        ReportCopyFormat { id: id.into(), name: id.into(), target: "entries".into(), delimiter: delimiter.into(), include_header, ai_prompt: ai_prompt.into(), columns }
    }

    pub fn report_entry(id: &str, minutes: i64, title: Option<&str>, note: Option<&str>, project: Option<(&str, &str, &str)>) -> ReportEntry {
        ReportEntry {
            id: id.into(),
            start: "2026-08-04T01:00:00Z".parse().unwrap(),
            end: "2026-08-04T02:30:00Z".parse().unwrap(),
            minutes,
            title: title.map(Into::into),
            note: note.map(Into::into),
            project: project.map(|(pid, name, client)| ReportEntryProject {
                id: pid.into(),
                name: name.into(),
                color: "#000000".into(),
                client: ReportEntryClient { id: format!("c-{client}"), name: client.into() },
            }),
            tags: vec![],
        }
    }
}
