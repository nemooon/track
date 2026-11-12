//! `track-cli`: the same subcommands, flags, JSON output and error codes as
//! src/cli/index.ts, but reading and writing the SQLite DB directly through track-core.
use serde_json::{Value, json};
use std::path::Path;
use track_core::ai_registration::AiRegistrationInput;
use track_core::guard::{data_dir_from_env, dev_guard};
use track_core::model::NewEntry;
use track_core::prepare::{self, PrepareEnv, entry_summary, jst_iso, parse_ms, QUARTER_MS};
use track_core::{CoreError, Db};

mod edit;

type Result<T> = std::result::Result<T, CoreError>;

pub(crate) fn fail<T>(code: &'static str, message: &str) -> Result<T> {
    Err(CoreError::code_msg(code, message))
}

/// First occurrence of `name` followed by its value (`option` in the TS CLI).
pub(crate) fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// Every value given for a repeatable flag.
pub(crate) fn options<'a>(args: &'a [String], name: &str) -> Vec<&'a str> {
    args.iter().enumerate().filter(|(_, a)| *a == name).filter_map(|(i, _)| args.get(i + 1)).map(String::as_str).collect()
}

fn usage() -> &'static str {
    "Track CLI

使い方:
  track-cli status
  track-cli projects
  track-cli list --from <ISO日時> --to <ISO日時>
  track-cli prepare [--source all|codex|claude] [--date YYYY-MM-DD]
                    [--scope day|session] [--idle-minutes 30]
                    [--session-id <ID>] [--session-file <ログパス>]
  track-cli create --start <ISO日時> --end <ISO日時> --title <タイトル>
                   [--project-id <ID>] [--note <メモ>] [--tag-id <ID> ...]
                   [--break-minutes <分>] --confirmed [--allow-overlap]
                   [--source codex|claude --session-id <ID> --request-id <UUID>]

  track-cli search [--project-id <ID>|none] [--query <語>] [--from <ISO日時>] [--to <ISO日時>] [--limit <N>]
  track-cli update --id <ID> [--id ...] [--project-id <ID>|--no-project] [--title <T>]
                   [--note <N>|--clear-note] [--tag-id <ID> ...|--clear-tags]
                   [--start <ISO日時>] [--end <ISO日時>] [--break-minutes <分>]
                   [--confirmed] [--source codex|claude]
  track-cli delete --id <ID> [--id ...] --confirmed [--source codex|claude]
  track-cli clients | tags | projects [--all]
  track-cli client create --name <名前> --confirmed
  track-cli client update --id <ID> [--name <名前>] [--archived true|false]
  track-cli project create --client-id <ID> --name <名前> [--color <#RRGGBB>] [--tag-id <ID> ...] --confirmed
  track-cli project update --id <ID> [--client-id <ID>] [--name <名前>] [--color <#RRGGBB>]
                           [--archived true|false] [--tag-id <ID> ...|--clear-tags]
  track-cli tag create --name <名前> [--color <#RRGGBB>]
  track-cli tag update --id <ID> [--name <名前>] [--color <#RRGGBB>]
  track-cli tag delete --id <ID> --confirmed
  track-cli history [--limit <N>]
  track-cli undo [--id <操作ID>]

createは利用者へ登録内容を提示し、明示的な確認を得た後に実行してください。
updateは10件以上、deleteは常に、クライアント・プロジェクトの作成とタグの削除は常に--confirmedが必要です。
書き込みは操作として記録され、undoで取り消せます（取り消しはAIの操作だけ）。
DBは$TRACK_DATA_DIR（既定 ~/.track）のtrack.dbを直接読み書きします。Trackアプリを起動しておく必要はありません。"
}

fn open_db() -> Result<Db> {
    let dir = data_dir_from_env();
    dev_guard(&dir)?;
    Db::open(&dir)
}

fn create(db: &mut Db, args: &[String]) -> Result<Value> {
    if !args.iter().any(|a| a == "--confirmed") {
        return fail("confirmation_required", "登録前に内容を提示し、利用者の確認を得てください。");
    }
    let start = option(args, "--start").ok_or_else(|| CoreError::code_msg("invalid_input", "--startが必要です。"))?;
    let end = option(args, "--end").ok_or_else(|| CoreError::code_msg("invalid_input", "--endが必要です。"))?;
    let title = option(args, "--title").ok_or_else(|| CoreError::code_msg("invalid_input", "--titleが必要です。"))?;
    let (s, e) = match (parse_ms(start), parse_ms(end)) {
        (Some(s), Some(e)) if e > s => (s, e),
        _ => return fail("invalid_interval", "endはstartより後にしてください。"),
    };
    if jst_iso(s)[..10] != jst_iso(e - 1)[..10] {
        return fail("crosses_jst_midnight", "JSTの同一日内に分けてください。");
    }
    if s % QUARTER_MS != 0 || e % QUARTER_MS != 0 {
        return fail("invalid_time_step", "開始・終了時刻は15分単位で指定してください。");
    }
    // JS String#length counts UTF-16 code units
    if title.encode_utf16().count() > 100 {
        return fail("title_too_long", "titleは100文字以内です。");
    }
    let source = option(args, "--source");
    let session_id = option(args, "--session-id");
    let request_id = option(args, "--request-id");
    if (source.is_some() || session_id.is_some() || request_id.is_some())
        && (!matches!(source, Some("codex" | "claude")) || session_id.is_none() || request_id.is_none())
    {
        return fail("invalid_input", "--source、--session-id、--request-idを一緒に指定してください。");
    }
    let allow_overlap = args.iter().any(|a| a == "--allow-overlap");
    if request_id.is_none() {
        let existing = prepare::overlaps(db, start, end)?;
        let duplicate = existing.iter().any(|x| {
            x["start"].as_str().and_then(parse_ms) == Some(s) && x["end"].as_str().and_then(parse_ms) == Some(e) && x["title"].as_str() == Some(title)
        });
        if duplicate {
            return fail("exact_duplicate", "同一エントリが登録済みです。");
        }
        if !existing.is_empty() && !allow_overlap {
            return Err(CoreError::with_details(
                "overlap_requires_confirmation",
                "既存エントリと重複します。重複を提示し、許可後に--allow-overlapを付けてください。",
                Value::Array(existing),
            ));
        }
    }
    let break_minutes = match option(args, "--break-minutes") {
        None => 0,
        Some(b) => b.parse::<i64>().map_err(|_| CoreError::code_msg("invalid_input", "--break-minutesは整数で指定してください。"))?,
    };
    let entry = NewEntry {
        project_id: option(args, "--project-id").map(String::from),
        start: chrono::DateTime::from_timestamp_millis(s).unwrap(),
        end: chrono::DateTime::from_timestamp_millis(e).unwrap(),
        title: Some(title.to_string()),
        note: option(args, "--note").map(String::from),
        tag_ids: options(args, "--tag-id").into_iter().map(String::from).collect(),
        break_minutes,
        ..Default::default()
    };
    let (created, op) = match request_id {
        Some(rid) => {
            let reg = AiRegistrationInput {
                request_id: rid.to_string(),
                source: source.unwrap().to_string(),
                session_id: session_id.unwrap().to_string(),
                allow_overlap,
            };
            let (e, _, op) = db.record_ai_entry(source.unwrap(), entry, reg)?;
            (e, op)
        }
        None => {
            let (e, op) = db.op_create_entry("cli", entry)?;
            (e, Some(op))
        }
    };
    Ok(json!({ "created": entry_summary(&created), "operation": op }))
}

fn prepare_cmd(db: &Db, args: &[String]) -> Result<Value> {
    let idle: f64 = option(args, "--idle-minutes").unwrap_or("30").parse().unwrap_or(f64::NAN);
    if !idle.is_finite() || idle < 1. {
        return fail("invalid_input", "--idle-minutesは1以上の数値にしてください。");
    }
    let idle = idle as i64;
    let session_id = option(args, "--session-id");
    let session_file = option(args, "--session-file");
    let scope = option(args, "--scope").unwrap_or(if session_id.is_some() || session_file.is_some() { "session" } else { "day" });
    let env = PrepareEnv::from_env();
    match scope {
        "day" => prepare::prepare_day(db, &env, option(args, "--date"), option(args, "--source").unwrap_or("all"), idle),
        "session" => prepare::prepare_session(db, &env, option(args, "--source"), session_id, session_file.map(Path::new), idle),
        _ => fail("invalid_scope", "--scope day|sessionを指定してください。"),
    }
}

enum Output {
    Text(&'static str),
    Json(Value),
}

fn run(args: &[String]) -> Result<Output> {
    let Some(command) = args.first().map(String::as_str).filter(|c| !matches!(*c, "help" | "--help" | "-h")) else {
        return Ok(Output::Text(usage()));
    };
    let rest = &args[1..];
    if !matches!(command, "status" | "projects" | "list" | "prepare" | "create" | "search" | "update" | "delete" | "clients" | "tags" | "client" | "project" | "tag" | "history" | "undo") {
        return fail("invalid_command", &format!("未対応のコマンドです: {command}"));
    }
    let mut db = open_db()?;
    let value = match command {
        "status" => json!({ "connected": true, "dbPath": db.path, "projectCount": db.projects(false)?.len() }),
        "projects" => json!({ "projects": db.projects(rest.iter().any(|a| a == "--all"))? }),
        "clients" => json!({ "clients": db.clients()? }),
        "tags" => json!({ "tags": db.tags()? }),
        "search" => edit::search(&db, rest)?,
        "update" => edit::update(&mut db, rest)?,
        "delete" => edit::delete(&mut db, rest)?,
        "client" => edit::client(&mut db, rest)?,
        "project" => edit::project(&mut db, rest)?,
        "tag" => edit::tag(&mut db, rest)?,
        "history" => edit::history(&db, rest)?,
        "undo" => edit::undo(&mut db, rest)?,
        "list" => {
            let from = option(rest, "--from").ok_or_else(|| CoreError::code_msg("invalid_input", "--fromが必要です。"))?;
            let to = option(rest, "--to").ok_or_else(|| CoreError::code_msg("invalid_input", "--toが必要です。"))?;
            json!({ "entries": prepare::overlaps(&db, from, to)? })
        }
        "prepare" => prepare_cmd(&db, rest)?,
        _ => create(&mut db, rest)?,
    };
    Ok(Output::Json(value))
}

fn with_ok(ok: bool, v: Value) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("ok".into(), Value::Bool(ok));
    if let Value::Object(m) = v {
        out.extend(m);
    }
    Value::Object(out)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(Output::Text(t)) => println!("{t}"),
        Ok(Output::Json(v)) => println!("{}", serde_json::to_string_pretty(&with_ok(true, v)).unwrap()),
        Err(e) => {
            let mut body = json!({ "error": e.code(), "message": e.to_string() });
            let details = e.details();
            if !details.is_null() {
                body["details"] = details;
            }
            println!("{}", serde_json::to_string_pretty(&with_ok(false, body)).unwrap());
            std::process::exit(1);
        }
    }
}
