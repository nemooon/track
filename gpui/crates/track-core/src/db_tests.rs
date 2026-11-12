use crate::guard::dev_guard;
use crate::{CoreError, Db};

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn fresh_db_applies_all_16_migrations() {
    let dir = tmp();
    let db = Db::open(dir.path()).unwrap();
    let n: i64 = db.conn().query_row("SELECT COUNT(*) FROM _local_migrations", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 16);
    for t in ["TimeEntry", "AiRegistration", "Settings", "Project", "Client", "Tag", "TagOnEntry", "TagOnProject"] {
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
