//! Shapes shared by the app, CLI and storage (same JSON field names as the Bun API).
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

/// How much a tag is used: projects it is on and entries that carry it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagUsage {
    pub tag_id: String,
    pub projects: i64,
    pub entries: i64,
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
    /// `direct` for a 直行直帰 day; everything else is `normal`.
    #[serde(default)]
    pub kind: EntryKind,
}

/// What a time entry is: an ordinary record, or a 直行直帰 day (one day of scheduled work for a
/// project, made from the calendar's day menu).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    #[default]
    Normal,
    Direct,
}

impl EntryKind {
    pub fn as_db(self) -> &'static str {
        match self {
            EntryKind::Normal => "normal",
            EntryKind::Direct => "direct",
        }
    }

    pub fn from_db(s: &str) -> Self {
        if s == "direct" { EntryKind::Direct } else { EntryKind::Normal }
    }
}

/// A day's leave. `None` is stored only on a holiday, to cancel its automatic leave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeaveKind {
    Full,
    Am,
    Pm,
    None,
}

impl LeaveKind {
    pub fn as_db(self) -> &'static str {
        match self {
            LeaveKind::Full => "full",
            LeaveKind::Am => "am",
            LeaveKind::Pm => "pm",
            LeaveKind::None => "none",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        Some(match s {
            "full" => LeaveKind::Full,
            "am" => LeaveKind::Am,
            "pm" => LeaveKind::Pm,
            "none" => LeaveKind::None,
            _ => return None,
        })
    }
}

/// A row of the `Leave` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Leave {
    pub date: chrono::NaiveDate,
    pub kind: LeaveKind,
}

/// What kind of leave a day in the report's 休暇 breakdown is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum LeaveDayKind {
    Full,
    Am,
    Pm,
    /// A holiday on a work day, counted as full leave without a row (its name).
    Holiday(String),
}

impl LeaveDayKind {
    /// 「休暇」「前半休」「後半休」「祝日 名前」.
    pub fn label(&self) -> String {
        match self {
            Self::Full => "休暇".into(),
            Self::Am => "前半休".into(),
            Self::Pm => "後半休".into(),
            Self::Holiday(name) => format!("祝日 {name}"),
        }
    }
}

/// One day of leave in a report period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LeaveDay {
    pub date: chrono::NaiveDate,
    pub kind: LeaveDayKind,
    pub minutes: i64,
}

/// `PATCH /api/settings`
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsPatch {
    pub work_start: Option<i64>,
    pub work_end: Option<i64>,
    pub work_days: Option<Vec<u32>>,
    pub weekly_report_template: Option<String>,
    pub report_copy_formats: Option<Vec<ReportCopyFormat>>,
}

/// `PATCH /api/clients/:id`
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClientPatch {
    pub name: Option<String>,
    pub archived: Option<bool>,
}

/// `PATCH /api/projects/:id`. `tag_ids` replaces the project's tags.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectPatch {
    pub client_id: Option<String>,
    pub name: Option<String>,
    pub color: Option<String>,
    pub archived: Option<bool>,
    pub tag_ids: Option<Vec<String>>,
}

/// `PATCH /api/tags/:id`
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TagPatch {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub ai_provider: String,
    #[serde(default)]
    pub ai_codex_executable: String,
    #[serde(default)]
    pub ai_codex_model: String,
    #[serde(default)]
    pub ai_command_executable: String,
    #[serde(default)]
    pub ai_command_args: Vec<String>,
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
    /// Add a 「休暇」 row (and the heatmap's leave) when no filter is set.
    pub include_leave: bool,
}


