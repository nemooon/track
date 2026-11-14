//! Ports of src/cli/index.test.ts (same inputs, same expectations).
use super::*;
use chrono::DateTime;
use std::fs;
use std::path::PathBuf;

fn ms(s: &str) -> i64 {
    DateTime::parse_from_rfc3339(s).unwrap().timestamp_millis()
}

fn stamp(clock: &str) -> i64 {
    ms(&format!("2026-10-01T{clock}:00+09:00"))
}

struct Fixture {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    file: PathBuf,
}

fn fixture(id: &str, timestamps: &[&str]) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    let file = dir.join(format!("{id}.jsonl"));
    let cwd = std::env::current_dir().unwrap();
    let mut lines = vec![serde_json::json!({"type": "session_meta", "payload": {"id": id, "cwd": cwd}}).to_string()];
    lines.extend(timestamps.iter().map(|t| serde_json::json!({ "timestamp": t }).to_string()));
    fs::write(&file, lines.join("\n")).unwrap();
    Fixture { _tmp: tmp, dir, file }
}

fn env_with(codex: &std::path::Path, claude: &std::path::Path, thread: Option<&str>, now: &str) -> PrepareEnv {
    PrepareEnv {
        now: ms(now),
        codex_root: codex.to_path_buf(),
        codex_archived_root: None,
        claude_root: claude.to_path_buf(),
        cwd: std::env::current_dir().unwrap(),
        codex_thread_id: thread.map(String::from),
    }
}

// --- AI activity candidates ---

#[test]
fn proposes_only_after_recorded_and_never_rounds_back() {
    let f = fixture("a", &["2026-10-01T10:00:00+09:00", "2026-10-01T10:20:00+09:00", "2026-10-01T10:35:00+09:00"]);
    let w = activity_window(&f.file, 30, ms("2026-10-01T10:40:00+09:00"), Some("2026-10-01T10:30:00+09:00")).unwrap().unwrap();
    assert_eq!((w["start"].as_str(), w["end"].as_str()), (Some("2026-10-01T10:30:00+09:00"), Some("2026-10-01T10:45:00+09:00")));
}

#[test]
fn no_new_activity_means_no_entry_from_now_alone() {
    let f = fixture("a", &["2026-10-01T10:20:00+09:00"]);
    assert!(activity_window(&f.file, 30, ms("2026-10-01T12:00:00+09:00"), Some("2026-10-01T10:30:00+09:00")).unwrap().is_none());
}

#[test]
fn old_log_tail_is_not_extended_to_now() {
    let f = fixture("a", &["2026-10-01T10:05:00+09:00", "2026-10-01T10:20:00+09:00"]);
    let w = activity_window(&f.file, 30, ms("2026-10-01T12:00:00+09:00"), None).unwrap().unwrap();
    assert_eq!((w["start"].as_str(), w["end"].as_str()), (Some("2026-10-01T10:00:00+09:00"), Some("2026-10-01T10:30:00+09:00")));
}

#[test]
fn splits_across_jst_midnight_with_distinct_request_ids() {
    let parts = registration_segments(Some(&serde_json::json!({"start": "2026-10-01T23:45:00+09:00", "end": "2026-10-02T00:30:00+09:00"})));
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0]["end"], "2026-10-02T00:00:00+09:00");
    assert_eq!(parts[1]["start"], parts[0]["end"]);
    assert_ne!(parts[0]["requestId"], parts[1]["requestId"]);
    assert!(registration_segments(None).is_empty());
}

// --- session selection ---

#[test]
fn missing_current_id_does_not_fall_back() {
    let f = fixture("session-a", &[]);
    let env = env_with(&f.dir, &f.dir, Some("missing"), "2026-10-01T12:00:00+09:00");
    assert_eq!(find_session(&env, Source::Codex, None, None).unwrap_err().code(), "session_not_found");
}

#[test]
fn multiple_sessions_in_same_folder_are_ambiguous() {
    let f = fixture("session-a", &[]);
    let cwd = std::env::current_dir().unwrap();
    fs::write(f.dir.join("session-b.jsonl"), serde_json::json!({"type": "session_meta", "payload": {"id": "session-b", "cwd": cwd}}).to_string()).unwrap();
    let env = env_with(&f.dir, &f.dir, None, "2026-10-01T12:00:00+09:00");
    assert_eq!(find_session(&env, Source::Codex, None, None).unwrap_err().code(), "ambiguous_session");
    assert_eq!(find_session(&env, Source::Codex, Some("session-b"), None).unwrap().id, "session-b");
}

#[test]
fn explicit_file_id_is_checked() {
    let f = fixture("session-a", &[]);
    let env = env_with(&f.dir, &f.dir, None, "2026-10-01T12:00:00+09:00");
    assert_eq!(find_session(&env, Source::Codex, Some("session-a"), Some(&f.file)).unwrap().file, f.file);
    assert_eq!(find_session(&env, Source::Codex, Some("other"), Some(&f.file)).unwrap_err().code(), "session_id_mismatch");
}

#[test]
fn claude_explicit_log_reads_session_id() {
    let f = fixture("session-a", &[]);
    fs::write(&f.file, serde_json::json!({"sessionId": "claude-id", "timestamp": "2026-10-01T10:00:00+09:00"}).to_string()).unwrap();
    let env = env_with(&f.dir, &f.dir, None, "2026-10-01T12:00:00+09:00");
    assert_eq!(find_session(&env, Source::Claude, Some("claude-id"), Some(&f.file)).unwrap().id, "claude-id");
}

// --- daily conversation candidates ---

const NOW: &str = "2026-10-01T16:00:00+09:00";

#[test]
fn daily_windows_split_on_long_breaks() {
    let w = daily_windows(&[stamp("10:04"), stamp("10:20"), stamp("14:10"), stamp("14:20")], day_bounds("2026-10-01").unwrap(), ms(NOW), 30);
    assert_eq!(w, vec![Interval { start: stamp("10:00"), end: stamp("10:30") }, Interval { start: stamp("14:00"), end: stamp("14:30") }]);
}

#[test]
fn daily_windows_use_jst_day_and_skip_future() {
    let w = daily_windows(
        &[ms("2026-09-30T15:05:00Z"), ms("2026-09-30T14:55:00Z"), ms("2026-10-01T15:05:00Z"), stamp("18:00")],
        day_bounds("2026-10-01").unwrap(),
        ms(NOW),
        30,
    );
    assert_eq!(w, vec![Interval { start: stamp("00:00"), end: stamp("00:15") }]);
    assert_eq!(day_bounds("2026-02-30").unwrap_err().code(), "invalid_day");
}

#[test]
fn coverage_removes_only_the_middle() {
    let i = |a: &str, b: &str| Interval { start: stamp(a), end: stamp(b) };
    assert_eq!(subtract_coverage(&[i("10:00", "11:00")], &[i("10:15", "10:30")]), vec![i("10:00", "10:15"), i("10:30", "11:00")]);
    assert!(subtract_coverage(&[i("10:00", "11:00")], &[i("09:00", "12:00")]).is_empty());
}

#[test]
fn coverage_keeps_quarter_boundaries() {
    let i = |a: &str, b: &str| Interval { start: stamp(a), end: stamp(b) };
    assert_eq!(subtract_coverage(&[i("10:00", "11:00")], &[i("10:17", "10:28")]), vec![i("10:00", "10:15"), i("10:30", "11:00")]);
}

#[test]
fn collects_codex_and_claude_and_skips_subagents() {
    let f = fixture("day-codex", &[]);
    let rows = [
        serde_json::json!({"type": "session_meta", "payload": {"id": "day-codex", "cwd": "/other-project"}}),
        serde_json::json!({"timestamp": "2026-10-01T10:04:00+09:00", "type": "event_msg", "payload": {"type": "user_message", "message": "検索画面を修正"}}),
        serde_json::json!({"timestamp": "2026-10-01T10:20:00+09:00", "type": "event_msg", "payload": {"type": "agent_message", "message": "検索を実装しました"}}),
    ];
    fs::write(&f.file, rows.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("\n")).unwrap();
    let claude = f.dir.join("claude-root");
    fs::create_dir_all(claude.join("project/subagents")).unwrap();
    let transcript = [
        serde_json::json!({"timestamp": "2026-10-01T11:05:00+09:00", "sessionId": "day-claude", "cwd": "/third-project", "type": "user", "message": {"content": "レポートを修正"}}),
        serde_json::json!({"timestamp": "2026-10-01T11:15:00+09:00", "sessionId": "day-claude", "type": "assistant", "message": {"content": [{"type": "text", "text": "修正完了"}]}}),
    ]
    .iter()
    .map(|r| r.to_string())
    .collect::<Vec<_>>()
    .join("\n");
    fs::write(claude.join("project/day-claude.jsonl"), &transcript).unwrap();
    fs::write(claude.join("project/subagents/agent.jsonl"), transcript.replace("day-claude", "agent")).unwrap();
    let env = env_with(&f.dir, &claude, Some("unrelated-current-thread"), NOW);
    let result = collect_daily_sessions(&env, &[Source::Codex, Source::Claude], "2026-10-01", 30).unwrap();
    assert_eq!(result.sessions.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["day-codex", "day-claude"]);
    assert_eq!(result.sessions[0].cwd.as_deref(), Some("/other-project"));
    assert_eq!(result.sessions[1].messages[0]["text"], "レポートを修正");
    assert!(collect_daily_sessions(&env, &[Source::Codex], "2026-09-30", 30).unwrap().sessions.is_empty());
}
