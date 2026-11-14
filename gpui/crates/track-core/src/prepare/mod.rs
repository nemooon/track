//! `track-cli prepare`: candidate entries from Codex / Claude Code conversation logs.
//! A line-for-line port of src/cli/index.ts (same windows, rounding, JSON shape and errors).
use crate::error::{CoreError, Result};
use crate::model::TimeEntry;
use crate::Db;
use chrono::{DateTime, FixedOffset, Utc};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const QUARTER_MS: i64 = 15 * 60_000;
const DAY_MS: i64 = 24 * 60 * 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Codex,
    Claude,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Codex => "codex",
            Source::Claude => "claude",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub start: i64,
    pub end: i64,
}

/// Where logs live and what "now" is (environment-driven in production, fixed in tests).
pub struct PrepareEnv {
    pub now: i64,
    pub codex_root: PathBuf,
    /// `~/.codex/archived_sessions`, only when `CODEX_SESSIONS_DIR` is unset.
    pub codex_archived_root: Option<PathBuf>,
    pub claude_root: PathBuf,
    pub cwd: PathBuf,
    pub codex_thread_id: Option<String>,
}

impl PrepareEnv {
    pub fn from_env() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let codex_dir = std::env::var_os("CODEX_SESSIONS_DIR").map(PathBuf::from);
        PrepareEnv {
            now: Utc::now().timestamp_millis(),
            codex_archived_root: codex_dir.is_none().then(|| home.join(".codex").join("archived_sessions")),
            codex_root: codex_dir.unwrap_or_else(|| home.join(".codex").join("sessions")),
            claude_root: std::env::var_os("TRACK_CLAUDE_PROJECTS_DIR").map(PathBuf::from).unwrap_or_else(|| home.join(".claude").join("projects")),
            cwd: std::env::current_dir().unwrap_or_default(),
            codex_thread_id: std::env::var("CODEX_THREAD_ID").ok(),
        }
    }

    fn root(&self, s: Source) -> &Path {
        match s {
            Source::Codex => &self.codex_root,
            Source::Claude => &self.claude_root,
        }
    }
}

fn jst() -> FixedOffset {
    FixedOffset::east_opt(9 * 3600).unwrap()
}

/// `YYYY-MM-DDTHH:mm:ss+09:00`
pub fn jst_iso(ms: i64) -> String {
    DateTime::from_timestamp_millis(ms).unwrap().with_timezone(&jst()).format("%Y-%m-%dT%H:%M:%S+09:00").to_string()
}

/// `Date.parse` for the ISO strings found in logs and our own output.
pub fn parse_ms(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s).ok().map(|t| t.timestamp_millis())
}

fn floor_q(ms: i64) -> i64 {
    ms.div_euclid(QUARTER_MS) * QUARTER_MS
}

fn ceil_q(ms: i64) -> i64 {
    -((-ms).div_euclid(QUARTER_MS)) * QUARTER_MS
}

pub fn json_lines(file: &Path) -> Result<Vec<Value>> {
    let text = std::fs::read_to_string(file)?;
    Ok(text.split('\n').filter(|l| !l.is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect())
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    // Filesystem order, like Bun's readdirSync (it does not sort); ties in the final
    // session order depend on it.
    let entries: Vec<_> = rd.flatten().collect();
    let mut out = Vec::new();
    for e in entries {
        let p = e.path();
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            out.extend(walk(&p));
        } else if p.to_string_lossy().ends_with(".jsonl") {
            out.push(p);
        }
    }
    out
}

fn timestamp_ms(item: &Value) -> Option<i64> {
    item.get("timestamp").and_then(Value::as_str).and_then(parse_ms)
}

fn session_meta(records: &[Value]) -> Option<&Value> {
    records.iter().find(|r| r.get("type").and_then(Value::as_str) == Some("session_meta")).and_then(|r| r.get("payload"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub id: String,
    pub file: PathBuf,
}

fn session_from_file(source: Source, file: &Path) -> Result<Session> {
    let records = json_lines(file)?;
    let id = match source {
        Source::Codex => session_meta(&records).and_then(|m| m.get("id")).and_then(Value::as_str).map(String::from),
        Source::Claude => records
            .iter()
            .find_map(|r| r.get("sessionId").and_then(Value::as_str).map(String::from))
            .or_else(|| file.file_stem().map(|s| s.to_string_lossy().into_owned())),
    };
    match id {
        Some(id) if !id.is_empty() && id.chars().count() <= 200 => Ok(Session { id, file: file.to_path_buf() }),
        _ => Err(CoreError::with_details("invalid_session", "ログからセッションIDを確認できません。", json!({ "file": file }))),
    }
}

pub fn find_session(env: &PrepareEnv, source: Source, session_id: Option<&str>, session_file: Option<&Path>) -> Result<Session> {
    let explicit_id: Option<String> = session_id.map(String::from).or_else(|| if source == Source::Codex { env.codex_thread_id.clone() } else { None });
    if let Some(file) = session_file {
        let file = if file.is_absolute() { file.to_path_buf() } else { env.cwd.join(file) };
        let session = session_from_file(source, &file)?;
        if explicit_id.as_ref().is_some_and(|id| *id != session.id) {
            return Err(CoreError::code_msg("session_id_mismatch", "指定したIDとログのセッションIDが一致しません。"));
        }
        return Ok(session);
    }
    let root = env.root(source);
    let files = if source == Source::Claude && explicit_id.is_none() {
        walk(&root.join(env.cwd.to_string_lossy().replace('/', "-")))
    } else {
        walk(root)
    };
    let cwd = env.cwd.to_string_lossy().into_owned();
    let mut candidates = Vec::new();
    for file in files {
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if let Some(id) = &explicit_id
            && !name.contains(id.as_str()) {
                continue;
            }
        let Ok(session) = session_from_file(source, &file) else { continue };
        if let Some(id) = &explicit_id {
            if session.id == *id {
                candidates.push(session);
            }
            continue;
        }
        if source == Source::Codex {
            let records = json_lines(&file).unwrap_or_default();
            if session_meta(&records).and_then(|m| m.get("cwd")).and_then(Value::as_str) != Some(cwd.as_str()) {
                continue;
            }
        }
        candidates.push(session);
    }
    match candidates.len() {
        0 => Err(CoreError::code_msg("session_not_found", "セッションを特定できません。--session-idまたは--session-fileを指定してください。")),
        1 => Ok(candidates.remove(0)),
        _ => Err(CoreError::with_details(
            "ambiguous_session",
            "複数のセッションがあります。--session-idまたは--session-fileを指定してください。",
            json!(candidates.iter().take(10).map(|c| json!({ "id": c.id, "file": c.file })).collect::<Vec<_>>()),
        )),
    }
}

pub fn activity_window(file: &Path, idle_minutes: i64, now: i64, recorded_until: Option<&str>) -> Result<Option<Value>> {
    let mut times: Vec<i64> = json_lines(file)?.iter().filter_map(timestamp_ms).collect();
    times.sort();
    if times.is_empty() {
        return Err(CoreError::code_msg("session_has_no_timestamps", "セッションに時刻情報がありません。"));
    }
    let cutoff = match recorded_until {
        Some(s) => Some(parse_ms(s).ok_or_else(|| CoreError::code_msg("invalid_progress", "登録済み時刻が不正です。"))?),
        None => None,
    };
    if let Some(c) = cutoff {
        times.retain(|t| *t > c);
    }
    let Some(&last) = times.last() else { return Ok(None) };
    let idle = idle_minutes * 60_000;
    if now >= last && now - last < idle {
        times.push(now);
    }
    let mut index = times.len() - 1;
    while index > 0 && times[index] - times[index - 1] < idle {
        index -= 1;
    }
    let start = floor_q(times[index]).max(cutoff.unwrap_or(i64::MIN));
    let mut end = ceil_q(*times.last().unwrap());
    if end <= start {
        end = start + QUARTER_MS;
    }
    Ok(Some(json!({
        "start": jst_iso(start),
        "end": jst_iso(end),
        "durationMinutes": (end - start) / 60_000,
        "crossesJstMidnight": jst_iso(start)[..10] != jst_iso(end - 1)[..10],
        "sessionFile": file,
        "recordedUntil": recorded_until,
    })))
}

pub fn registration_segments(window: Option<&Value>) -> Vec<Value> {
    let Some(w) = window else { return vec![] };
    let (Some(mut start), Some(end)) = (w["start"].as_str().and_then(parse_ms), w["end"].as_str().and_then(parse_ms)) else { return vec![] };
    let mut out = Vec::new();
    while start < end {
        let day_start = parse_ms(&format!("{}T00:00:00+09:00", &jst_iso(start)[..10])).unwrap();
        let seg_end = end.min(day_start + DAY_MS);
        out.push(json!({ "requestId": uuid::Uuid::new_v4().to_string(), "start": jst_iso(start), "end": jst_iso(seg_end) }));
        start = seg_end;
    }
    out
}

pub fn day_bounds(day: &str) -> Result<Interval> {
    let ok_shape = day.len() == 10 && day.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
    if !ok_shape {
        return Err(CoreError::code_msg("invalid_day", "--dateはYYYY-MM-DDで指定してください。"));
    }
    let start = parse_ms(&format!("{day}T00:00:00+09:00")).filter(|s| jst_iso(*s)[..10] == *day);
    match start {
        Some(start) => Ok(Interval { start, end: start + DAY_MS }),
        None => Err(CoreError::code_msg("invalid_day", "指定した日付が不正です。")),
    }
}

pub fn daily_windows(times: &[i64], bounds: Interval, now: i64, idle_minutes: i64) -> Vec<Interval> {
    let idle = idle_minutes * 60_000;
    let mut sorted: Vec<i64> = times.iter().copied().filter(|t| *t >= bounds.start && *t < bounds.end && *t <= now).collect();
    sorted.sort();
    sorted.dedup();
    let mut groups: Vec<Vec<i64>> = Vec::new();
    for t in sorted {
        match groups.last_mut() {
            Some(g) if t - *g.last().unwrap() < idle => g.push(t),
            _ => groups.push(vec![t]),
        }
    }
    groups
        .into_iter()
        .map(|g| {
            let (first, last) = (g[0], *g.last().unwrap());
            let until = if now >= last && now - last < idle { now.min(bounds.end) } else { last };
            let start = bounds.start.max(floor_q(first));
            let end = bounds.end.min((start + QUARTER_MS).max(ceil_q(until)));
            Interval { start, end }
        })
        .collect()
}

fn conversation_message(item: &Value) -> Option<Value> {
    let ty = item.get("type").and_then(Value::as_str);
    let payload = item.get("payload");
    let ptype = payload.and_then(|p| p.get("type")).and_then(Value::as_str);
    let (role, content): (Option<&str>, Option<&Value>) = if ty == Some("event_msg") && matches!(ptype, Some("user_message" | "agent_message")) {
        (Some(if ptype == Some("user_message") { "user" } else { "assistant" }), payload.and_then(|p| p.get("message")))
    } else if ty == Some("response_item") && ptype == Some("message") {
        (payload.and_then(|p| p.get("role")).and_then(Value::as_str), payload.and_then(|p| p.get("content")))
    } else if matches!(ty, Some("user" | "assistant")) {
        (ty, item.get("message").and_then(|m| m.get("content")))
    } else {
        (None, None)
    };
    let role = role.filter(|r| matches!(*r, "user" | "assistant"))?;
    let text = match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts.iter().filter_map(|p| p.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    };
    if text.trim().is_empty() {
        return None;
    }
    // JS slices UTF-16 code units; slice by chars (identical for BMP text)
    let text: String = text.chars().take(800).collect();
    Some(json!({ "timestamp": item.get("timestamp").cloned().unwrap_or(Value::Null), "role": role, "text": text }))
}

pub struct DailySession {
    pub id: String,
    pub file: PathBuf,
    pub source: Source,
    pub cwd: Option<String>,
    pub messages: Vec<Value>,
    pub windows: Vec<Interval>,
}

pub struct Collected {
    pub sessions: Vec<DailySession>,
    pub warnings: Vec<Value>,
}

pub fn collect_daily_sessions(env: &PrepareEnv, sources: &[Source], day: &str, idle_minutes: i64) -> Result<Collected> {
    let bounds = day_bounds(day)?;
    let mut order: Vec<String> = Vec::new();
    let mut sessions: HashMap<String, (DailySession, Vec<i64>)> = HashMap::new();
    let mut warnings = Vec::new();
    for &source in sources {
        let mut files = walk(env.root(source));
        if source == Source::Codex
            && let Some(archived) = &env.codex_archived_root {
                files.extend(walk(archived));
            }
        for file in files {
            if file.components().any(|c| c.as_os_str() == "subagents") {
                continue;
            }
            let step = (|| -> Result<()> {
                let records = json_lines(&file)?;
                let today: Vec<&Value> = records
                    .iter()
                    .filter(|r| timestamp_ms(r).is_some_and(|t| t >= bounds.start && t < bounds.end && t <= env.now))
                    .collect();
                if today.is_empty() {
                    return Ok(());
                }
                let meta = session_meta(&records);
                if source == Source::Codex && meta.and_then(|m| m.get("source")).is_some_and(|o| o.is_object() && o.get("subagent").is_some()) {
                    return Ok(());
                }
                let found = session_from_file(source, &file)?;
                let key = format!("{}:{}", source.as_str(), found.id);
                let messages: Vec<Value> = today.iter().filter_map(|r| conversation_message(r)).collect();
                let times: Vec<i64> = today.iter().filter_map(|r| timestamp_ms(r)).collect();
                if let Some((s, t)) = sessions.get_mut(&key) {
                    t.extend(times);
                    s.messages.extend(messages);
                } else {
                    let cwd = meta
                        .and_then(|m| m.get("cwd"))
                        .or_else(|| records.iter().find_map(|r| r.get("cwd").filter(|c| c.is_string())))
                        .and_then(Value::as_str)
                        .map(String::from);
                    order.push(key.clone());
                    sessions.insert(key, (DailySession { id: found.id, file: found.file, source, cwd, messages, windows: vec![] }, times));
                }
                Ok(())
            })();
            if let Err(e) = step {
                warnings.push(json!({ "file": file, "error": e.to_string() }));
            }
        }
    }
    let mut out: Vec<DailySession> = order
        .into_iter()
        .filter_map(|k| sessions.remove(&k))
        .map(|(mut s, times)| {
            // dedupe by JSON (last occurrence wins, first position kept, like a JS Map), then sort by time
            let mut seen: Vec<(String, Value)> = Vec::new();
            for m in s.messages.drain(..) {
                let k = m.to_string();
                match seen.iter_mut().find(|(sk, _)| *sk == k) {
                    Some(slot) => slot.1 = m,
                    None => seen.push((k, m)),
                }
            }
            let mut msgs: Vec<Value> = seen.into_iter().map(|(_, m)| m).collect();
            msgs.sort_by_key(|m| m["timestamp"].as_str().and_then(parse_ms).unwrap_or(0));
            if msgs.len() > 20 {
                let tail = msgs.split_off(msgs.len() - 17);
                msgs.truncate(3);
                msgs.extend(tail);
            }
            s.messages = msgs;
            s.windows = daily_windows(&times, bounds, env.now, idle_minutes);
            s
        })
        .collect();
    out.sort_by_key(|s| s.windows.first().map(|w| w.start).unwrap_or(i64::MAX));
    Ok(Collected { sessions: out, warnings })
}

pub fn subtract_coverage(windows: &[Interval], covered: &[Interval]) -> Vec<Interval> {
    let mut remaining = windows.to_vec();
    for taken in covered {
        remaining = remaining
            .into_iter()
            .flat_map(|w| {
                if taken.end <= w.start || taken.start >= w.end {
                    return vec![w];
                }
                let mut pieces = Vec::new();
                let before = floor_q(taken.start);
                let after = ceil_q(taken.end);
                if before > w.start {
                    pieces.push(Interval { start: w.start, end: before });
                }
                if after < w.end {
                    pieces.push(Interval { start: after, end: w.end });
                }
                pieces
            })
            .collect();
    }
    remaining
}

fn js_iso(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// `entrySummary` (start/end in the server's ISO form).
pub fn entry_summary(e: &TimeEntry) -> Value {
    json!({
        "id": e.id,
        "start": js_iso(e.start),
        "end": js_iso(e.end),
        "title": e.title,
        "project": e.project.as_ref().map(|p| p.name.clone()),
        "client": e.project.as_ref().map(|p| p.client.name.clone()),
    })
}

pub fn overlaps(db: &Db, start: &str, end: &str) -> Result<Vec<Value>> {
    let (Some(s), Some(e)) = (parse_ms(start), parse_ms(end)) else {
        return Err(CoreError::code_msg("invalid_input", "日時が不正です。"));
    };
    let s = DateTime::from_timestamp_millis(s).unwrap();
    let e = DateTime::from_timestamp_millis(e).unwrap();
    Ok(db.entries(s, e)?.iter().map(entry_summary).collect())
}

pub fn prepare_day(db: &Db, env: &PrepareEnv, date: Option<&str>, selected: &str, idle_minutes: i64) -> Result<Value> {
    let sources: Vec<Source> = match selected {
        "all" => vec![Source::Codex, Source::Claude],
        "codex" => vec![Source::Codex],
        "claude" => vec![Source::Claude],
        _ => return Err(CoreError::code_msg("invalid_source", "--source all|codex|claudeを指定してください。")),
    };
    let day = date.map(String::from).unwrap_or_else(|| jst_iso(env.now)[..10].to_string());
    let bounds = day_bounds(&day)?;
    let collected = collect_daily_sessions(env, &sources, &day, idle_minutes)?;
    let existing = overlaps(db, &jst_iso(bounds.start), &jst_iso(bounds.end))?;
    let projects = serde_json::to_value(db.projects(false)?).unwrap();
    let mut segments: Vec<Value> = Vec::new();
    for s in &collected.sessions {
        let mut covered: Vec<Interval> = existing
            .iter()
            .filter_map(|e| Some(Interval { start: parse_ms(e["start"].as_str()?)?, end: parse_ms(e["end"].as_str()?)? }))
            .collect();
        covered.extend(db.ai_coverage(s.source.as_str(), &s.id)?.into_iter().map(|(from, until)| Interval {
            start: from.map(|f| f.timestamp_millis()).unwrap_or(bounds.start),
            end: until.timestamp_millis(),
        }));
        for w in subtract_coverage(&s.windows, &covered) {
            segments.push(json!({
                "requestId": uuid::Uuid::new_v4().to_string(),
                "source": s.source.as_str(),
                "sessionId": s.id,
                "start": jst_iso(w.start),
                "end": jst_iso(w.end),
                "durationMinutes": (w.end - w.start) / 60_000,
            }));
        }
    }
    let span = |v: &Value| (parse_ms(v["start"].as_str().unwrap()).unwrap(), parse_ms(v["end"].as_str().unwrap()).unwrap());
    segments.sort_by_key(|s| span(s).0);
    let spans: Vec<(i64, i64)> = segments.iter().map(span).collect();
    let ids: Vec<Value> = segments.iter().map(|s| s["requestId"].clone()).collect();
    for (i, seg) in segments.iter_mut().enumerate() {
        let parallel: Vec<Value> = (0..spans.len()).filter(|&j| j != i && spans[j].0 < spans[i].1 && spans[j].1 > spans[i].0).map(|j| ids[j].clone()).collect();
        seg["parallelWith"] = Value::Array(parallel);
    }
    let mut union: Vec<(i64, i64)> = Vec::new();
    for &(s, e) in &spans {
        match union.last_mut() {
            Some(prior) if s <= prior.1 => prior.1 = prior.1.max(e),
            _ => union.push((s, e)),
        }
    }
    let total: i64 = union.iter().map(|(s, e)| (e - s) / 60_000).sum();
    Ok(json!({
        "scope": "day",
        "date": day,
        "timezone": "Asia/Tokyo",
        "source": selected,
        "roundingMinutes": 15,
        "segments": segments,
        "totalCandidateMinutes": total,
        "sessions": collected.sessions.iter().map(|s| json!({
            "sessionId": s.id, "source": s.source.as_str(), "sessionFile": s.file, "cwd": s.cwd, "messages": s.messages,
        })).collect::<Vec<_>>(),
        "projects": projects,
        "existing": existing,
        "warnings": collected.warnings,
    }))
}

pub fn prepare_session(db: &Db, env: &PrepareEnv, source: Option<&str>, session_id: Option<&str>, session_file: Option<&Path>, idle_minutes: i64) -> Result<Value> {
    let source = match source {
        Some("codex") => Source::Codex,
        Some("claude") => Source::Claude,
        _ => return Err(CoreError::code_msg("invalid_source", "会話単位では--source codex|claudeが必要です。")),
    };
    let session = find_session(env, source, session_id, session_file)?;
    let recorded_until = db.ai_progress(source.as_str(), &session.id)?.map(js_iso);
    let window = activity_window(&session.file, idle_minutes, env.now, recorded_until.as_deref())?;
    let projects = serde_json::to_value(db.projects(false)?).unwrap();
    let ov = match &window {
        Some(w) => overlaps(db, w["start"].as_str().unwrap(), w["end"].as_str().unwrap())?,
        None => vec![],
    };
    Ok(json!({
        "scope": "session",
        "roundingMinutes": 15,
        "source": source.as_str(),
        "sessionId": session.id,
        "segments": registration_segments(window.as_ref()),
        "window": window,
        "projects": projects,
        "overlaps": ov,
    }))
}

#[cfg(test)]
mod prepare_tests;

/// Same day candidates as the Bun CLI on the real logs (read-only) and the copy DB.
/// Requires `npm run local` on 8787 with TRACK_DATA_DIR=$HOME/.track-gpui. Run with --ignored.
#[cfg(test)]
mod live_tests {
    use super::*;

    fn normalize(v: &Value) -> Value {
        let segs: Vec<Value> = v["segments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| json!([s["source"], s["sessionId"], s["start"], s["end"], s["durationMinutes"], s["parallelWith"].as_array().unwrap().len()]))
            .collect();
        json!({
            "segments": segs,
            "total": v["totalCandidateMinutes"],
            "sessions": v["sessions"],
            "existing": v["existing"],
            "warnings": v["warnings"].as_array().unwrap().len(),
            "projectIds": v["projects"].as_array().unwrap().iter().map(|p| p["id"].clone()).collect::<Vec<_>>(),
        })
    }

    #[test]
    #[ignore]
    fn matches_bun_cli_on_real_logs() {
        let day = "2026-11-11";
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let out = std::process::Command::new("bun")
            .args(["src/cli/index.ts", "prepare", "--date", day])
            .env("TRACK_API_BASE", "http://127.0.0.1:8787/api")
            .current_dir(&repo)
            .output()
            .unwrap();
        let theirs: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(theirs["ok"], true, "{theirs}");
        let db = Db::open(&dirs::home_dir().unwrap().join(".track-gpui")).unwrap();
        let ours = prepare_day(&db, &PrepareEnv::from_env(), Some(day), "all", 30).unwrap();
        let (a, b) = (normalize(&ours), normalize(&theirs));
        for k in ["segments", "total", "sessions", "existing", "warnings", "projectIds"] {
            assert_eq!(a[k], b[k], "field {k}");
        }
        println!("segments={} sessions={}", a["segments"].as_array().unwrap().len(), a["sessions"].as_array().unwrap().len());
    }
}
