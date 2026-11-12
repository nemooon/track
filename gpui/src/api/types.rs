use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagLink {
    pub tag_id: String,
    pub tag: Tag,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub archived: bool,
    pub client: Client,
    #[serde(default)]
    pub tags: Vec<TagLink>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: String,
    pub project_id: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub title: Option<String>,
    pub note: Option<String>,
    pub project: Option<Project>,
    #[serde(default)]
    pub tags: Vec<TagLink>,
    pub external_event_id: Option<String>,
    pub external_event_source: Option<String>,
    #[serde(default)]
    pub break_minutes: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub tag_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_event_source: Option<String>,
    pub break_minutes: i64,
}

/// PATCH body. Outer Option = "send this field"; inner Option = JSON null.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub break_minutes: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportDurationFormat {
    HoursMinutes,
    Japanese,
    DecimalWithUnit,
    Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportCopyField {
    Date,
    Start,
    End,
    Client,
    Project,
    Title,
    Note,
    Tags,
    Duration,
    DurationMinutes,
    Percentage,
    Category,
    Summary,
    EntryCount,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReportCopyColumn {
    #[serde(rename_all = "camelCase")]
    Field {
        id: String,
        field: ReportCopyField,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_format: Option<ReportDurationFormat>,
    },
    Blank {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    Ai { id: String, label: String, prompt: String },
}

impl ReportCopyColumn {
    pub fn id(&self) -> &str {
        match self {
            Self::Field { id, .. } | Self::Blank { id, .. } | Self::Ai { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCopyFormat {
    pub id: String,
    pub name: String,
    pub target: String,
    pub delimiter: String,
    pub include_header: bool,
    #[serde(default)]
    pub ai_prompt: String,
    pub columns: Vec<ReportCopyColumn>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub work_start: i64,
    pub work_end: i64,
    pub work_days: Vec<u32>,
    #[serde(default)]
    pub weekly_report_template: String,
    #[serde(default)]
    pub report_copy_formats: Vec<ReportCopyFormat>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub ai_provider: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRow {
    pub key: String,
    pub label: String,
    pub color: Option<String>,
    pub total_minutes: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DateSpan {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportResponse {
    pub rows: Vec<ReportRow>,
    pub total_minutes: i64,
    pub range: DateSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportEntryProject {
    pub id: String,
    pub name: String,
    pub color: String,
    pub client: ReportEntryClient,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportEntryClient {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportEntry {
    pub id: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub minutes: i64,
    pub title: Option<String>,
    pub note: Option<String>,
    pub project: Option<ReportEntryProject>,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEntriesResponse {
    pub entries: Vec<ReportEntry>,
    pub total_minutes: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportRange {
    Week,
    Month,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupBy {
    Client,
    Project,
    Tag,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportQuery {
    pub range: ReportRange,
    /// Local midnight of the anchor date.
    pub anchor: DateTime<Utc>,
    pub group_by: GroupBy,
    pub client_ids: Vec<String>,
    pub project_ids: Vec<String>,
    pub tag_ids: Vec<String>,
}

impl ReportQuery {
    pub fn params(&self, with_group: bool) -> Vec<(&'static str, String)> {
        let mut p = vec![
            ("range", match self.range { ReportRange::Week => "week", ReportRange::Month => "month" }.to_string()),
            ("anchor", self.anchor.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        ];
        if with_group {
            p.push(("groupBy", match self.group_by { GroupBy::Client => "client", GroupBy::Project => "project", GroupBy::Tag => "tag" }.to_string()));
        }
        for (k, v) in [("clientIds", &self.client_ids), ("projectIds", &self.project_ids), ("tagIds", &self.tag_ids)] {
            if !v.is_empty() {
                p.push((k, v.join(",")));
            }
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_time_entry() {
        let json = r##"{"id":"e1","projectId":"p1","start":"2026-11-12T01:00:00.000Z","end":"2026-11-12T02:00:00.000Z",
          "title":"定例","note":null,"breakMinutes":30,"externalEventId":null,"externalEventSource":null,
          "project":{"id":"p1","clientId":"c1","name":"P","color":"#123456","archived":false,
             "client":{"id":"c1","name":"C","archived":false},"tags":[{"tagId":"t1","tag":{"id":"t1","name":"T","color":"#000000"}}]},
          "tags":[{"tagId":"t2","tag":{"id":"t2","name":"U","color":"#ffffff"}}],"createdAt":"x","updatedAt":"y"}"##;
        let e: TimeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.break_minutes, 30);
        assert_eq!(e.project.as_ref().unwrap().client.name, "C");
        assert_eq!(e.project.as_ref().unwrap().tags[0].tag.id, "t1");
        assert_eq!(e.tags[0].tag.name, "U");
        assert_eq!(e.start.to_rfc3339(), "2026-11-12T01:00:00+00:00");
    }

    #[test]
    fn deserializes_copy_format_columns() {
        let a: ReportCopyColumn = serde_json::from_str(r#"{"kind":"ai","id":"a","label":"L","prompt":"P"}"#).unwrap();
        assert_eq!(a, ReportCopyColumn::Ai { id: "a".into(), label: "L".into(), prompt: "P".into() });
        let b: ReportCopyColumn = serde_json::from_str(r#"{"kind":"blank","id":"b"}"#).unwrap();
        assert_eq!(b, ReportCopyColumn::Blank { id: "b".into(), label: None });
        let f: ReportCopyColumn =
            serde_json::from_str(r#"{"kind":"field","id":"f","field":"durationMinutes","durationFormat":"decimal-with-unit"}"#).unwrap();
        assert_eq!(
            f,
            ReportCopyColumn::Field {
                id: "f".into(),
                field: ReportCopyField::DurationMinutes,
                label: None,
                duration_format: Some(ReportDurationFormat::DecimalWithUnit)
            }
        );
    }
}
