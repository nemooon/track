use serde_json::Value;
use std::process::Command;

struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        // create the DB and one project through the core
        let mut db = track_core::Db::open(dir.path()).unwrap();
        let c = db.create_client("Acme").unwrap();
        db.create_project(&c, "Alpha", "#3b82f6").unwrap();
        Env { dir }
    }

    fn run(&self, args: &[&str]) -> (i32, Value) {
        self.run_tz(args, None)
    }

    fn run_tz(&self, args: &[&str], tz: Option<&str>) -> (i32, Value) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_track-cli"));
        cmd.args(args).env("TRACK_DATA_DIR", self.dir.path()).env("TRACK_DEV_GUARD_ROOT", self.dir.path());
        if let Some(tz) = tz {
            cmd.env("TZ", tz);
        }
        let out = cmd.output().unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        let json = serde_json::from_str(&text).unwrap_or(Value::String(text));
        (out.status.code().unwrap(), json)
    }
}

const RID: &str = "6f1c1f8e-2a6b-4c1d-9d3e-1a2b3c4d5e6f";

fn create_args<'a>(start: &'a str, end: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut v = vec!["create", "--start", start, "--end", end, "--title", "設計"];
    v.extend_from_slice(extra);
    v
}

#[test]
fn help_prints_usage_exit_0() {
    let e = Env::new();
    let (code, out) = e.run(&["help"]);
    assert_eq!(code, 0);
    assert!(out.as_str().unwrap().contains("track-cli prepare"));
}

#[test]
fn unknown_command_is_invalid_command() {
    let (code, out) = Env::new().run(&["nope"]);
    assert_eq!((code, out["ok"].clone(), out["error"].clone()), (1, Value::Bool(false), Value::from("invalid_command")));
}

#[test]
fn status_reports_db_path_and_project_count() {
    let e = Env::new();
    let (code, out) = e.run(&["status"]);
    assert_eq!(code, 0);
    assert_eq!(out["ok"], true);
    assert_eq!(out["connected"], true);
    assert_eq!(out["projectCount"], 1);
    assert!(out["dbPath"].as_str().unwrap().ends_with("track.db"));
}

#[test]
fn dev_guard_rejects_other_data_dirs() {
    let e = Env::new();
    let out = Command::new(env!("CARGO_BIN_EXE_track-cli"))
        .arg("status")
        .env("TRACK_DATA_DIR", e.dir.path())
        .env_remove("TRACK_DEV_GUARD_ROOT")
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!((out.status.code(), v["error"].as_str()), (Some(1), Some("dev_guard")));
}

#[test]
fn create_requires_confirmed() {
    let (code, out) = Env::new().run(&create_args("2026-11-12T10:00:00+09:00", "2026-11-12T11:00:00+09:00", &[]));
    assert_eq!((code, out["error"].as_str()), (1, Some("confirmation_required")));
}

#[test]
fn create_rejects_off_quarter() {
    let (_, out) = Env::new().run(&create_args("2026-11-12T10:05:00+09:00", "2026-11-12T11:00:00+09:00", &["--confirmed"]));
    assert_eq!(out["error"], "invalid_time_step");
}

#[test]
fn create_rejects_cross_jst_midnight_even_if_local_same_day() {
    let (_, out) = Env::new().run_tz(&create_args("2026-11-12T23:00:00+09:00", "2026-11-13T01:00:00+09:00", &["--confirmed"]), Some("UTC"));
    assert_eq!(out["error"], "crosses_jst_midnight");
}

#[test]
fn create_with_request_id_is_idempotent() {
    let e = Env::new();
    let args = create_args("2026-11-12T10:00:00+09:00", "2026-11-12T11:00:00+09:00", &["--confirmed", "--source", "codex", "--session-id", "s1", "--request-id", RID]);
    let (c1, o1) = e.run(&args);
    let (c2, o2) = e.run(&args);
    assert_eq!((c1, c2), (0, 0), "{o1} {o2}");
    assert_eq!(o1["created"]["id"], o2["created"]["id"]);
    assert_eq!(o1["created"]["title"], "設計");
    let (_, list) = e.run(&["list", "--from", "2026-11-12T00:00:00+09:00", "--to", "2026-11-13T00:00:00+09:00"]);
    assert_eq!(list["entries"].as_array().unwrap().len(), 1);
}

#[test]
fn create_without_request_id_reports_overlap() {
    let e = Env::new();
    assert_eq!(e.run(&create_args("2026-11-12T10:00:00+09:00", "2026-11-12T11:00:00+09:00", &["--confirmed"])).0, 0);
    let (_, dup) = e.run(&create_args("2026-11-12T10:00:00+09:00", "2026-11-12T11:00:00+09:00", &["--confirmed"]));
    assert_eq!(dup["error"], "exact_duplicate");
    let (_, ov) = e.run(&["create", "--start", "2026-11-12T10:30:00+09:00", "--end", "2026-11-12T11:30:00+09:00", "--title", "別", "--confirmed"]);
    assert_eq!(ov["error"], "overlap_requires_confirmation");
    assert_eq!(ov["details"][0]["title"], "設計");
    let (code, _) = e.run(&["create", "--start", "2026-11-12T10:30:00+09:00", "--end", "2026-11-12T11:30:00+09:00", "--title", "別", "--confirmed", "--allow-overlap"]);
    assert_eq!(code, 0);
}

#[test]
fn errors_go_to_stdout_with_exit_1() {
    let out = Command::new(env!("CARGO_BIN_EXE_track-cli")).args(["list"]).env("TRACK_DATA_DIR", "/nonexistent").env("TRACK_DEV_GUARD_ROOT", "/nonexistent").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.is_empty());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert!(v["message"].is_string());
}

#[test]
fn projects_lists_active_projects() {
    let (_, out) = Env::new().run(&["projects"]);
    assert_eq!(out["projects"][0]["name"], "Alpha");
    assert_eq!(out["projects"][0]["client"]["name"], "Acme");
}
