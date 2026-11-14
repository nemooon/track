//! Edits the AI makes through the CLI. Each call is one recorded operation (see `ops`), so it
//! can be undone from the conversation or the app.
use crate::ai_registration::AiRegistrationInput;
use crate::error::{CoreError, Result};
use crate::model::{Client, ClientPatch, EntryPatch, NewEntry, Project, ProjectPatch, Tag, TagPatch, TimeEntry};
use crate::ops::{Operation, Table};
use crate::Db;
use chrono::{DateTime, Utc};

/// Project colours for the settings screen and the CLI. The first ten are the Tauri app's
/// `PROJECT_COLORS` (src/client/lib/utils.ts), kept first so automatic choices do not change.
pub const PROJECT_COLORS: [&str; 20] = [
    "#ef4444", "#f97316", "#eab308", "#22c55e", "#06b6d4", "#3b82f6", "#6366f1", "#a855f7", "#ec4899", "#64748b",
    "#f59e0b", "#84cc16", "#10b981", "#14b8a6", "#0ea5e9", "#8b5cf6", "#d946ef", "#f43f5e", "#b45309", "#78716c",
];

/// The colour the settings screen would give the next project.
pub fn next_project_color(existing: usize) -> &'static str {
    PROJECT_COLORS[existing % PROJECT_COLORS.len()]
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntrySearch {
    /// `"none"` = entries without a project.
    pub project_id: Option<String>,
    /// Literal text in the title or note.
    pub query: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

fn invalid(message: &str) -> CoreError {
    CoreError::code_msg("invalid_input", message)
}

fn unique(ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

fn changed_fields(p: &EntryPatch) -> String {
    let mut f = Vec::new();
    if p.project_id.is_some() {
        f.push("プロジェクト");
    }
    if p.title.is_some() {
        f.push("タイトル");
    }
    if p.note.is_some() {
        f.push("メモ");
    }
    if p.tag_ids.is_some() {
        f.push("タグ");
    }
    if p.start.is_some() || p.end.is_some() {
        f.push("時刻");
    }
    if p.break_minutes.is_some() {
        f.push("休憩");
    }
    f.join("・")
}

impl Db {
    fn watch_entry(&self, rec: &mut crate::ops::Recorder, id: &str) -> Result<()> {
        rec.watch(self, Table::TimeEntry, "id", id)?;
        rec.watch(self, Table::TagOnEntry, "entryId", id)
    }

    /// Changes every entry in `ids` the same way (start/end only for a single entry).
    pub fn edit_entries(&mut self, source: &str, ids: &[String], patch: EntryPatch) -> Result<(Vec<TimeEntry>, Operation)> {
        let ids = unique(ids);
        if ids.is_empty() {
            return Err(invalid("変更する工数を指定してください。"));
        }
        if ids.len() > 1 && (patch.start.is_some() || patch.end.is_some()) {
            return Err(invalid("開始・終了は1件ずつ変更してください。"));
        }
        let fields = changed_fields(&patch);
        if fields.is_empty() {
            return Err(invalid("変更する項目を指定してください。"));
        }
        let to_project = match &patch.project_id {
            Some(Some(p)) => format!("（→ {}）", self.project(p).map(|p| p.name).unwrap_or_else(|_| p.clone())),
            Some(None) => "（→ プロジェクトなし）".into(),
            None => String::new(),
        };
        self.with_operation(source, move |db, rec| {
            let mut out = Vec::with_capacity(ids.len());
            for id in &ids {
                db.watch_entry(rec, id)?;
                out.push(db.update_entry(id, patch.clone())?);
            }
            let n = out.len() as i64;
            Ok((out, format!("{n}件の工数の{fields}を変更{to_project}"), n))
        })
    }

    pub fn remove_entries(&mut self, source: &str, ids: &[String]) -> Result<((), Operation)> {
        let ids = unique(ids);
        if ids.is_empty() {
            return Err(invalid("削除する工数を指定してください。"));
        }
        self.with_operation(source, move |db, rec| {
            for id in &ids {
                db.watch_entry(rec, id)?;
                // The registration's entryId becomes NULL on delete; keep it to put the link back.
                let regs: Vec<String> = db
                    .conn()
                    .prepare(r#"SELECT requestId FROM "AiRegistration" WHERE entryId = ?1"#)?
                    .query_map([id], |r| r.get(0))?
                    .collect::<std::result::Result<_, _>>()?;
                for r in regs {
                    rec.watch(db, Table::AiRegistration, "requestId", &r)?;
                }
                db.delete_entry(id)?;
            }
            let n = ids.len() as i64;
            Ok(((), format!("{n}件の工数を削除"), n))
        })
    }

    pub fn op_create_entry(&mut self, source: &str, input: NewEntry) -> Result<(TimeEntry, Operation)> {
        self.with_operation(source, |db, rec| {
            let e = db.create_entry(input)?;
            rec.created(Table::TimeEntry, "id", &e.id);
            rec.created(Table::TagOnEntry, "entryId", &e.id);
            let summary = format!("工数を記録（{}）", e.title.clone().unwrap_or_default());
            Ok((e, summary, 1))
        })
    }

    /// AI registration as an operation. A replay of an earlier identical request changes
    /// nothing and records no operation.
    pub fn record_ai_entry(&mut self, source: &str, input: NewEntry, reg: AiRegistrationInput) -> Result<(TimeEntry, bool, Option<Operation>)> {
        let request_id = reg.request_id.clone();
        let ((e, replay), op) = self.with_optional_operation(source, |db, rec| {
            let (e, replay) = db.create_ai_entry(input, reg)?;
            if replay {
                rec.nothing_changed();
            }
            rec.created(Table::TimeEntry, "id", &e.id);
            rec.created(Table::TagOnEntry, "entryId", &e.id);
            rec.created(Table::AiRegistration, "requestId", &request_id);
            let summary = format!("工数を記録（{}）", e.title.clone().unwrap_or_default());
            Ok(((e, replay), summary, 1))
        })?;
        Ok((e, replay, op))
    }

    pub fn op_create_client(&mut self, source: &str, name: &str) -> Result<(Client, Operation)> {
        self.with_operation(source, |db, rec| {
            let id = db.create_client(name)?;
            rec.created(Table::Client, "id", &id);
            Ok((db.client(&id)?, format!("クライアント「{name}」を作成"), 1))
        })
    }

    pub fn op_update_client(&mut self, source: &str, id: &str, patch: ClientPatch) -> Result<(Client, Operation)> {
        self.with_operation(source, |db, rec| {
            rec.watch(db, Table::Client, "id", id)?;
            rec.watch(db, Table::Project, "clientId", id)?;
            let c = db.update_client(id, patch)?;
            Ok((c.clone(), format!("クライアント「{}」を変更", c.name), 1))
        })
    }

    pub fn op_create_project(&mut self, source: &str, client_id: &str, name: &str, color: &str, tag_ids: &[String]) -> Result<(Project, Operation)> {
        self.with_operation(source, |db, rec| {
            let id = db.create_project_with_tags(client_id, name, color, tag_ids)?;
            rec.created(Table::Project, "id", &id);
            rec.created(Table::TagOnProject, "projectId", &id);
            Ok((db.project(&id)?, format!("プロジェクト「{name}」を作成"), 1))
        })
    }

    pub fn op_update_project(&mut self, source: &str, id: &str, patch: ProjectPatch) -> Result<(Project, Operation)> {
        self.with_operation(source, |db, rec| {
            rec.watch(db, Table::Project, "id", id)?;
            rec.watch(db, Table::TagOnProject, "projectId", id)?;
            let p = db.update_project(id, patch)?;
            Ok((p.clone(), format!("プロジェクト「{}」を変更", p.name), 1))
        })
    }

    pub fn op_create_tag(&mut self, source: &str, name: &str, color: &str) -> Result<(Tag, Operation)> {
        self.with_operation(source, |db, rec| {
            let id = db.create_tag(name, color)?;
            rec.created(Table::Tag, "id", &id);
            let t = db.tags()?.into_iter().find(|t| t.id == id).expect("just created");
            Ok((t, format!("タグ「{name}」を作成"), 1))
        })
    }

    pub fn op_update_tag(&mut self, source: &str, id: &str, patch: TagPatch) -> Result<(Tag, Operation)> {
        self.with_operation(source, |db, rec| {
            rec.watch(db, Table::Tag, "id", id)?;
            let t = db.update_tag(id, patch)?;
            Ok((t.clone(), format!("タグ「{}」を変更", t.name), 1))
        })
    }

    pub fn op_delete_tag(&mut self, source: &str, id: &str) -> Result<((), Operation)> {
        self.with_operation(source, |db, rec| {
            let name = db.tags()?.into_iter().find(|t| t.id == id).map(|t| t.name).unwrap_or_default();
            rec.watch(db, Table::Tag, "id", id)?;
            rec.watch(db, Table::TagOnEntry, "tagId", id)?;
            rec.watch(db, Table::TagOnProject, "tagId", id)?;
            db.delete_tag(id)?;
            Ok(((), format!("タグ「{name}」を削除"), 1))
        })
    }
}
