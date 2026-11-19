use serde_json::Value;
use std::process::Command;
use track_core::model::NewEntry;

struct Env {
    dir: tempfile::TempDir,
    project: String,
    other: String,
    client: String,
}

impl Env {
    /// Client "Acme" with projects "Alpha" and "Beta"; `n` entries in Alpha on 2026-11-12 (one per 15 minutes from 09:00 JST).
    fn new(n: usize) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut db = track_core::Db::open(dir.path()).unwrap();
        let client = db.create_client("Acme").unwrap();
        let project = db.create_project(&client, "Alpha", "#3b82f6").unwrap();
        let other = db.create_project(&client, "Beta", "#ef4444").unwrap();
        for i in 0..n as i64 {
            let start = chrono::DateTime::parse_from_rfc3339("2026-11-12T00:00:00Z").unwrap().to_utc() + chrono::Duration::minutes(15 * i);
            db.create_entry(NewEntry { project_id: Some(project.clone()), start, end: start + chrono::Duration::minutes(15), title: Some(format!("作業{i}")), ..Default::default() }).unwrap();
        }
        Env { dir, project, other, client }
    }

    fn db(&self) -> track_core::Db {
        track_core::Db::open(self.dir.path()).unwrap()
    }

    fn run(&self, args: &[&str]) -> (i32, Value) {
        let out = Command::new(env!("CARGO_BIN_EXE_track-cli"))
            .args(args)
            .env("TRACK_DATA_DIR", self.dir.path())
            .env("TRACK_DEV_GUARD_ROOT", self.dir.path())
            .output()
            .unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        (out.status.code().unwrap(), serde_json::from_str(&text).unwrap_or(Value::String(text)))
    }

    fn ids(&self, n: usize) -> Vec<String> {
        let (_, out) = self.run(&["search", "--project-id", &self.project]);
        out["entries"].as_array().unwrap().iter().take(n).map(|e| e["id"].as_str().unwrap().to_string()).collect()
    }

    fn in_project(&self, project: &str) -> usize {
        self.db().search_entries(&track_core::edit::EntrySearch { project_id: Some(project.into()), ..Default::default() }, 1000).unwrap().len()
    }
}

fn with_ids<'a>(mut args: Vec<&'a str>, ids: &'a [String]) -> Vec<&'a str> {
    for id in ids {
        args.push("--id");
        args.push(id);
    }
    args
}

#[test]
fn update_ten_entries_requires_confirmation() {
    let e = Env::new(10);
    let ids = e.ids(10);
    let args = with_ids(vec!["update", "--project-id", &e.other, "--source", "codex"], &ids);
    let (code, out) = e.run(&args);
    assert_eq!((code, out["error"].as_str()), (1, Some("confirmation_required")));
    assert_eq!(out["details"]["count"], 10);
    assert_eq!(out["details"]["entries"].as_array().unwrap().len(), 10);
    assert_eq!(e.in_project(&e.other), 0);
    let mut args = args.clone();
    args.push("--confirmed");
    let (code, out) = e.run(&args);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out["operation"]["count"], 10);
    assert_eq!(out["operation"]["source"], "codex");
    assert_eq!(e.in_project(&e.other), 10);
}

#[test]
fn update_nine_entries_runs_without_confirmation() {
    let e = Env::new(9);
    let ids = e.ids(9);
    let (code, out) = e.run(&with_ids(vec!["update", "--project-id", &e.other], &ids));
    assert_eq!(code, 0, "{out}");
    assert_eq!(out["operation"]["source"], "cli");
    assert_eq!(e.in_project(&e.other), 9);
}

#[test]
fn delete_requires_confirmation_even_for_one() {
    let e = Env::new(1);
    let ids = e.ids(1);
    let (code, out) = e.run(&with_ids(vec!["delete"], &ids));
    assert_eq!((code, out["error"].as_str()), (1, Some("confirmation_required")));
    assert_eq!(e.in_project(&e.project), 1);
    let (code, _) = e.run(&with_ids(vec!["delete", "--confirmed"], &ids));
    assert_eq!(code, 0);
    assert_eq!(e.in_project(&e.project), 0);
}

#[test]
fn project_create_requires_confirmation() {
    let e = Env::new(0);
    let (code, out) = e.run(&["project", "create", "--client-id", &e.client, "--name", "Gamma"]);
    assert_eq!((code, out["error"].as_str()), (1, Some("confirmation_required")));
    let (code, out) = e.run(&["project", "create", "--client-id", &e.client, "--name", "Gamma", "--confirmed"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out["project"]["name"], "Gamma");
    assert_eq!(out["project"]["color"], track_core::edit::PROJECT_COLORS[2], "third project gets the third colour");
}

#[test]
fn client_create_requires_confirmation() {
    let e = Env::new(0);
    let (code, out) = e.run(&["client", "create", "--name", "Newco"]);
    assert_eq!((code, out["error"].as_str()), (1, Some("confirmation_required")));
    let (code, out) = e.run(&["client", "create", "--name", "Newco", "--confirmed"]);
    assert_eq!((code, out["client"]["name"].as_str()), (0, Some("Newco")));
}

#[test]
fn tag_delete_requires_confirmation() {
    let e = Env::new(0);
    let (code, out) = e.run(&["tag", "create", "--name", "red"]);
    assert_eq!(code, 0, "{out}");
    let id = out["tag"]["id"].as_str().unwrap().to_string();
    let (code, out) = e.run(&["tag", "delete", "--id", &id]);
    assert_eq!((code, out["error"].as_str()), (1, Some("confirmation_required")));
    assert_eq!(e.run(&["tag", "delete", "--id", &id, "--confirmed"]).0, 0);
    assert!(e.run(&["tags"]).1["tags"].as_array().unwrap().is_empty());
}

#[test]
fn undo_after_update_restores_and_history_shows_undone() {
    let e = Env::new(2);
    let ids = e.ids(2);
    let (_, out) = e.run(&with_ids(vec!["update", "--title", "まとめ"], &ids));
    let op = out["operation"]["id"].as_i64().unwrap();
    let (code, out) = e.run(&["undo"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out["operation"]["id"], op);
    let titles: Vec<String> = e.db().search_entries(&track_core::edit::EntrySearch { project_id: Some(e.project.clone()), ..Default::default() }, 10).unwrap().into_iter().map(|x| x.title.unwrap()).collect();
    assert!(titles.iter().all(|t| t.starts_with("作業")));
    let (_, hist) = e.run(&["history"]);
    assert!(hist["operations"][0]["undoneAt"].is_string());
    assert_eq!(e.run(&["undo", "--id", &op.to_string()]).1["error"], "already_undone");
}

#[test]
fn undo_conflict_is_reported_with_changed_rows() {
    let e = Env::new(1);
    let ids = e.ids(1);
    e.run(&with_ids(vec!["update", "--title", "AI"], &ids));
    e.run(&with_ids(vec!["update", "--title", "後から"], &ids));
    let (_, hist) = e.run(&["history"]);
    let first = hist["operations"][1]["id"].as_i64().unwrap().to_string();
    let (code, out) = e.run(&["undo", "--id", &first]);
    assert_eq!((code, out["error"].as_str()), (1, Some("undo_conflict")));
    assert_eq!(out["details"]["changed"][0]["key"], ids[0].as_str());
}

#[test]
fn search_requires_a_condition() {
    let e = Env::new(1);
    let (code, out) = e.run(&["search"]);
    assert_eq!((code, out["error"].as_str()), (1, Some("invalid_input")));
    let (code, out) = e.run(&["search", "--query", "作業"]);
    assert_eq!(code, 0);
    assert_eq!(out["entries"][0]["projectId"], e.project.as_str());
}

#[test]
fn create_records_an_operation_that_undo_removes() {
    let e = Env::new(0);
    let (code, out) = e.run(&["create", "--start", "2026-11-12T01:00:00Z", "--end", "2026-11-12T02:00:00Z", "--title", "記録", "--confirmed"]);
    assert_eq!(code, 0, "{out}");
    assert!(out["operation"]["id"].is_i64());
    assert_eq!(e.run(&["undo"]).0, 0);
    assert!(e.db().search_entries(&track_core::edit::EntrySearch { query: Some("記録".into()), ..Default::default() }, 10).unwrap().is_empty());
}

#[test]
fn confirmed_given_as_an_option_value_does_not_count() {
    let e = Env::new(10);
    let ids = e.ids(10);
    let (code, out) = e.run(&with_ids(vec!["update", "--title", "--confirmed"], &ids));
    assert_eq!((code, out["error"].as_str()), (1, Some("confirmation_required")), "{out}");
    let (code, out) = e.run(&with_ids(vec!["delete", "--note", "--confirmed"], &ids[..1]));
    assert_eq!((code, out["error"].as_str()), (1, Some("confirmation_required")), "{out}");
    let (code, out) = e.run(&["create", "--start", "2026-11-12T03:00:00Z", "--end", "2026-11-12T04:00:00Z", "--title", "--confirmed"]);
    assert_eq!((code, out["error"].as_str()), (1, Some("confirmation_required")), "{out}");
}

#[test]
fn delete_with_unknown_id_changes_nothing() {
    let e = Env::new(2);
    let ids = e.ids(2);
    let mut args = with_ids(vec!["delete", "--confirmed"], &ids);
    args.extend(["--id", "missing"]);
    let (code, out) = e.run(&args);
    assert_eq!((code, out["error"].as_str()), (1, Some("not_found")));
    assert_eq!(e.in_project(&e.project), 2);
    assert!(e.run(&["history"]).1["operations"].as_array().unwrap().is_empty());
}
