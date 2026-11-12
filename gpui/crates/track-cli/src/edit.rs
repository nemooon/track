//! Editing commands for the AI: entries in bulk, clients, projects and tags, plus history and
//! undo. Every write is one recorded operation (track_core::ops).
use crate::{Result, fail, option, options};
use serde_json::{Value, json};
use track_core::edit::{EntrySearch, next_project_color};
use track_core::model::{ClientPatch, EntryPatch, ProjectPatch, TagPatch, TimeEntry};
use track_core::prepare::{entry_summary, parse_ms};
use track_core::{CoreError, Db};

/// Bulk entry changes from this many entries on need the user's confirmation.
pub const CONFIRM_FROM: usize = 10;

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn required<'a>(args: &'a [String], name: &str) -> Result<&'a str> {
    option(args, name).ok_or_else(|| CoreError::code_msg("invalid_input", format!("{name}が必要です。")))
}

fn time(args: &[String], name: &str) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    match option(args, name) {
        None => Ok(None),
        Some(s) => parse_ms(s)
            .and_then(chrono::DateTime::from_timestamp_millis)
            .map(Some)
            .ok_or_else(|| CoreError::code_msg("invalid_input", format!("{name}はISO日時で指定してください。"))),
    }
}

fn bool_opt(args: &[String], name: &str) -> Result<Option<bool>> {
    match option(args, name) {
        None => Ok(None),
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        Some(_) => fail("invalid_input", &format!("{name}はtrueかfalseで指定してください。")),
    }
}

fn source(args: &[String]) -> Result<&str> {
    match option(args, "--source") {
        None => Ok("cli"),
        Some(s @ ("codex" | "claude")) => Ok(s),
        Some(_) => fail("invalid_input", "--sourceはcodexかclaudeです。"),
    }
}

fn tag_ids(args: &[String]) -> Option<Vec<String>> {
    if flag(args, "--clear-tags") {
        Some(vec![])
    } else {
        let t: Vec<String> = options(args, "--tag-id").into_iter().map(String::from).collect();
        if t.is_empty() { None } else { Some(t) }
    }
}

/// `entry_summary` plus what an edit needs to see.
pub fn entry_detail(e: &TimeEntry) -> Value {
    let mut v = entry_summary(e);
    v["projectId"] = json!(e.project_id);
    v["note"] = json!(e.note);
    v["breakMinutes"] = json!(e.break_minutes);
    v["tags"] = json!(e.tags.iter().map(|t| json!({ "id": t.tag_id, "name": t.tag.name })).collect::<Vec<_>>());
    v
}

fn unique_ids(args: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in options(args, "--id") {
        if !out.iter().any(|x| x == id) {
            out.push(id.to_string());
        }
    }
    out
}

/// Stops with `confirmation_required`, listing the entries the user should see first.
fn confirm_entries(db: &Db, ids: &[String], message: String) -> Result<()> {
    let entries: Vec<Value> = ids.iter().filter_map(|id| db.entry(id).ok().flatten()).map(|e| entry_detail(&e)).collect();
    Err(CoreError::with_details("confirmation_required", message, json!({ "count": ids.len(), "entries": entries })))
}

pub fn search(db: &Db, args: &[String]) -> Result<Value> {
    let q = EntrySearch { project_id: option(args, "--project-id").map(String::from), query: option(args, "--query").map(String::from), from: time(args, "--from")?, to: time(args, "--to")? };
    if q == EntrySearch::default() {
        return fail("invalid_input", "--project-id、--query、--from、--toのどれかを指定してください。");
    }
    let limit = match option(args, "--limit") {
        None => 200,
        Some(l) => l.parse::<i64>().map_err(|_| CoreError::code_msg("invalid_input", "--limitは整数で指定してください。"))?,
    };
    let entries: Vec<Value> = db.search_entries(&q, limit)?.iter().map(entry_detail).collect();
    Ok(json!({ "entries": entries }))
}

pub fn update(db: &mut Db, args: &[String]) -> Result<Value> {
    let ids = unique_ids(args);
    if ids.is_empty() {
        return fail("invalid_input", "--idが必要です。");
    }
    let patch = EntryPatch {
        project_id: if flag(args, "--no-project") { Some(None) } else { option(args, "--project-id").map(|p| Some(p.to_string())) },
        title: option(args, "--title").map(|t| Some(t.to_string())),
        note: if flag(args, "--clear-note") { Some(None) } else { option(args, "--note").map(|n| Some(n.to_string())) },
        tag_ids: tag_ids(args),
        start: time(args, "--start")?,
        end: time(args, "--end")?,
        break_minutes: match option(args, "--break-minutes") {
            None => None,
            Some(b) => Some(b.parse::<i64>().map_err(|_| CoreError::code_msg("invalid_input", "--break-minutesは整数で指定してください。"))?),
        },
    };
    let src = source(args)?;
    if ids.len() >= CONFIRM_FROM && !flag(args, "--confirmed") {
        let n = ids.len();
        confirm_entries(db, &ids, format!("{n}件の工数を変更します。一覧を利用者に示して許可を得てから--confirmedを付けてください。"))?;
    }
    let (entries, op) = db.edit_entries(src, &ids, patch)?;
    Ok(json!({ "updated": entries.iter().map(entry_detail).collect::<Vec<_>>(), "operation": op }))
}

pub fn delete(db: &mut Db, args: &[String]) -> Result<Value> {
    let ids = unique_ids(args);
    if ids.is_empty() {
        return fail("invalid_input", "--idが必要です。");
    }
    let src = source(args)?;
    if !flag(args, "--confirmed") {
        let n = ids.len();
        confirm_entries(db, &ids, format!("{n}件の工数を削除します。一覧を利用者に示して許可を得てから--confirmedを付けてください。"))?;
    }
    let ((), op) = db.remove_entries(src, &ids)?;
    Ok(json!({ "deleted": ids.len(), "operation": op }))
}

fn confirm_create(args: &[String], what: &str, name: &str) -> Result<()> {
    if flag(args, "--confirmed") {
        Ok(())
    } else {
        Err(CoreError::with_details(
            "confirmation_required",
            format!("{what}「{name}」を作成します。利用者に確認してから--confirmedを付けてください。"),
            json!({ "name": name }),
        ))
    }
}

pub fn client(db: &mut Db, args: &[String]) -> Result<Value> {
    let src = source(args)?;
    match args.first().map(String::as_str) {
        Some("create") => {
            let name = required(args, "--name")?;
            confirm_create(args, "クライアント", name)?;
            let (c, op) = db.op_create_client(src, name)?;
            Ok(json!({ "client": c, "operation": op }))
        }
        Some("update") => {
            let id = required(args, "--id")?;
            let patch = ClientPatch { name: option(args, "--name").map(String::from), archived: bool_opt(args, "--archived")? };
            let (c, op) = db.op_update_client(src, id, patch)?;
            Ok(json!({ "client": c, "operation": op }))
        }
        _ => fail("invalid_command", "client create|update を指定してください。"),
    }
}

pub fn project(db: &mut Db, args: &[String]) -> Result<Value> {
    let src = source(args)?;
    match args.first().map(String::as_str) {
        Some("create") => {
            let client_id = required(args, "--client-id")?;
            let name = required(args, "--name")?;
            confirm_create(args, "プロジェクト", name)?;
            let color = match option(args, "--color") {
                Some(c) => c.to_string(),
                None => next_project_color(db.projects(true)?.len()).to_string(),
            };
            let tags = tag_ids(args).unwrap_or_default();
            let (p, op) = db.op_create_project(src, client_id, name, &color, &tags)?;
            Ok(json!({ "project": p, "operation": op }))
        }
        Some("update") => {
            let id = required(args, "--id")?;
            let patch = ProjectPatch {
                client_id: option(args, "--client-id").map(String::from),
                name: option(args, "--name").map(String::from),
                color: option(args, "--color").map(String::from),
                archived: bool_opt(args, "--archived")?,
                tag_ids: tag_ids(args),
            };
            let (p, op) = db.op_update_project(src, id, patch)?;
            Ok(json!({ "project": p, "operation": op }))
        }
        _ => fail("invalid_command", "project create|update を指定してください。"),
    }
}

pub fn tag(db: &mut Db, args: &[String]) -> Result<Value> {
    let src = source(args)?;
    match args.first().map(String::as_str) {
        Some("create") => {
            let name = required(args, "--name")?;
            let color = match option(args, "--color") {
                Some(c) => c.to_string(),
                None => next_project_color(db.tags()?.len()).to_string(),
            };
            let (t, op) = db.op_create_tag(src, name, &color)?;
            Ok(json!({ "tag": t, "operation": op }))
        }
        Some("update") => {
            let id = required(args, "--id")?;
            let patch = TagPatch { name: option(args, "--name").map(String::from), color: option(args, "--color").map(String::from) };
            let (t, op) = db.op_update_tag(src, id, patch)?;
            Ok(json!({ "tag": t, "operation": op }))
        }
        Some("delete") => {
            let id = required(args, "--id")?;
            if !flag(args, "--confirmed") {
                let name = db.tags()?.into_iter().find(|t| t.id == id).map(|t| t.name).unwrap_or_default();
                return Err(CoreError::with_details(
                    "confirmation_required",
                    format!("タグ「{name}」を削除します（付いている工数・プロジェクトからも外れます）。利用者に確認してから--confirmedを付けてください。"),
                    json!({ "name": name }),
                ));
            }
            let ((), op) = db.op_delete_tag(src, id)?;
            Ok(json!({ "operation": op }))
        }
        _ => fail("invalid_command", "tag create|update|delete を指定してください。"),
    }
}

pub fn history(db: &Db, args: &[String]) -> Result<Value> {
    let limit = match option(args, "--limit") {
        None => 20,
        Some(l) => l.parse::<i64>().map_err(|_| CoreError::code_msg("invalid_input", "--limitは整数で指定してください。"))?,
    };
    Ok(json!({ "operations": db.operations(limit)? }))
}

pub fn undo(db: &mut Db, args: &[String]) -> Result<Value> {
    let id = match option(args, "--id") {
        None => None,
        Some(i) => Some(i.parse::<i64>().map_err(|_| CoreError::code_msg("invalid_input", "--idは操作IDの数値です。"))?),
    };
    let r = db.undo_operation(id)?;
    Ok(json!({ "operation": r.operation }))
}
