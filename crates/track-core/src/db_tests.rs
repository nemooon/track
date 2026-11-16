use crate::guard::dev_guard;
use crate::{CoreError, Db};

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn fresh_db_applies_all_17_migrations() {
    let dir = tmp();
    let db = Db::open(dir.path()).unwrap();
    let n: i64 = db.conn().query_row("SELECT COUNT(*) FROM _local_migrations", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 17);
    for t in ["TimeEntry", "AiRegistration", "Settings", "Project", "Client", "Tag", "TagOnEntry", "TagOnProject", "Leave"] {
        let c: i64 = db.conn().query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?", [t], |r| r.get(0)).unwrap();
        assert_eq!(c, 1, "{t}");
    }
    let mode: String = db.conn().query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode, "wal");
    assert!(!db.read_only);
}

#[test]
fn reopen_is_idempotent() {
    let dir = tmp();
    drop(Db::open(dir.path()).unwrap());
    let db = Db::open(dir.path()).unwrap();
    assert!(db.applied_now.is_empty());
}

#[test]
fn creates_default_settings_row() {
    let dir = tmp();
    let db = Db::open(dir.path()).unwrap();
    let (s, e, d): (i64, i64, String) =
        db.conn().query_row(r#"SELECT workStart, workEnd, workDays FROM "Settings""#, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!((s, e, d.as_str()), (600, 1110, "1,2,3,4,5"));
    let n: i64 = db.conn().query_row(r#"SELECT COUNT(*) FROM "Settings""#, [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    drop(db);
    let db = Db::open(dir.path()).unwrap();
    let n: i64 = db.conn().query_row(r#"SELECT COUNT(*) FROM "Settings""#, [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "no second Settings row on reopen");
}

#[test]
fn refuses_unknown_migrations() {
    let dir = tmp();
    {
        let db = Db::open(dir.path()).unwrap();
        db.conn().execute(r#"INSERT INTO "_local_migrations"(name) VALUES ('9999_future.sql')"#, []).unwrap();
    }
    let db = Db::open(dir.path()).unwrap();
    assert!(db.read_only);
    assert_eq!(db.ensure_writable().unwrap_err().code(), "schema_too_new");
}

#[test]
fn dev_guard_accepts_only_home_copy() {
    let home = dirs::home_dir().unwrap();
    assert!(dev_guard(&home.join(".track-gpui")).is_ok());
    for bad in [std::path::PathBuf::from("~/.track-gpui"), "/tmp/x/.track-gpui".into(), home.join(".track"), home.join(".track-gpui/../.track")] {
        let e = dev_guard(&bad).unwrap_err();
        assert_eq!(e.code(), "dev_guard", "{bad:?}");
    }
}

#[test]
fn error_codes() {
    assert_eq!(CoreError::code_msg("invalid_input", "x").code(), "invalid_input");
}

/// Real copy DB (run with --ignored): must already be fully migrated.
#[test]
#[ignore]
fn opens_copy_db_without_migrating() {
    let dir = dirs::home_dir().unwrap().join(".track-gpui");
    let db = Db::open(&dir).unwrap();
    assert!(db.applied_now.is_empty(), "applied: {:?}", db.applied_now);
    assert!(!db.read_only);
}

#[test]
fn a_db_from_before_leave_gets_the_column_and_table() {
    let dir = tmp();
    let id = {
        let mut db = Db::open(dir.path()).unwrap();
        let local = crate::repo::repo_tests::local;
        let id = db.create_entry(crate::model::NewEntry { start: local(12, 9, 0), end: local(12, 10, 0), title: Some("x".into()), ..Default::default() }).unwrap().id;
        // Back to the shape before 0017.
        db.conn().execute_batch(r#"DELETE FROM _local_migrations WHERE name = '0017_leave_and_direct.sql'; DROP TABLE "Leave"; ALTER TABLE "TimeEntry" DROP COLUMN "kind";"#).unwrap();
        id
    };
    let db = Db::open(dir.path()).unwrap();
    let n: i64 = db.conn().query_row(r#"SELECT COUNT(*) FROM "Leave""#, [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
    assert_eq!(db.entry(&id).unwrap().unwrap().kind, crate::model::EntryKind::Normal, "existing entries are normal");
}
