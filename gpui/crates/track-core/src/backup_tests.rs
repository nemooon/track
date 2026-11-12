use crate::backup::*;
use crate::config::Config;
use crate::model::*;
use crate::repo::repo_tests::local;
use crate::Db;

/// A data dir and a backup dir, both under one dev root.
fn setup() -> (tempfile::TempDir, Db, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let db = Db::open(&root.path().join("data")).unwrap();
    let backups = root.path().join("exports");
    (root, db, backups)
}

fn add(db: &mut Db, title: &str) {
    db.create_entry(NewEntry { start: local(12, 9, 0), end: local(12, 10, 0), title: Some(title.into()), ..Default::default() }).unwrap();
}

fn titles(db: &Db) -> Vec<String> {
    db.entries(local(1, 0, 0), local(30, 0, 0)).unwrap().into_iter().filter_map(|e| e.title).collect()
}

#[test]
fn snapshot_is_consistent_single_file() {
    let (root, mut db, dir) = setup();
    add(&mut db, "a");
    let s = db.snapshot(&dir, false, root.path()).unwrap();
    assert!(s.name.starts_with("track-backup-") && s.name.ends_with(".db"), "{}", s.name);
    assert!(!s.auto && s.bytes > 0);
    assert!(!dir.join(format!("{}-wal", s.name)).exists());
    let copy = rusqlite::Connection::open(&s.path).unwrap();
    let n: i64 = copy.query_row(r#"SELECT COUNT(*) FROM "TimeEntry""#, [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    // Same second: a suffix avoids VACUUM INTO's "file exists" error.
    let s2 = db.snapshot(&dir, false, root.path()).unwrap();
    assert_ne!(s.path, s2.path);
    assert_eq!(list_snapshots(&dir).len(), 2);
}

#[test]
fn prune_keeps_manual_backups() {
    let (root, db, dir) = setup();
    db.snapshot(&dir, false, root.path()).unwrap();
    for _ in 0..3 {
        db.snapshot(&dir, true, root.path()).unwrap();
    }
    std::fs::write(dir.join("other.db"), b"x").unwrap();
    assert_eq!(prune_snapshots(&dir, 1), 2);
    let left = list_snapshots(&dir);
    assert_eq!(left.iter().filter(|s| s.auto).count(), 1);
    assert_eq!(left.iter().filter(|s| !s.auto).count(), 1);
    assert!(dir.join("other.db").exists());
}

#[test]
fn auto_backup_respects_interval() {
    let (root, db, dir) = setup();
    let mut cfg = Config::defaults(root.path());
    cfg.export_dir = dir.clone();
    cfg.backup_keep = 2;
    assert!(db.maybe_auto_backup(&cfg, root.path()).unwrap().is_some());
    assert!(db.maybe_auto_backup(&cfg, root.path()).unwrap().is_none(), "within the interval");
    cfg.backup_interval_hours = 0;
    assert!(db.maybe_auto_backup(&cfg, root.path()).unwrap().is_none(), "disabled");
}

#[test]
fn validate_rejects_non_track_db() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("x.db");
    rusqlite::Connection::open(&p).unwrap().execute_batch("CREATE TABLE t(a)").unwrap();
    assert!(validate_snapshot(&p).unwrap_err().contains("Track の DB ではない"));
    std::fs::write(d.path().join("junk.db"), b"not sqlite at all, definitely not").unwrap();
    assert!(validate_snapshot(&d.path().join("junk.db")).is_err());
    assert_eq!(validate_snapshot(&d.path().join("missing.db")).unwrap_err(), "ファイルが見つかりません");
}

#[test]
fn restore_replaces_and_reopens() {
    let (root, mut db, dir) = setup();
    add(&mut db, "before");
    let snap = db.snapshot(&dir, false, root.path()).unwrap();
    add(&mut db, "after");
    let counts = validate_snapshot(&snap.path).unwrap();
    assert_eq!(counts.get("TimeEntry"), Some(&1));
    let r = db.restore(&snap.path, &dir, root.path()).unwrap();
    assert!(r.safety.path.exists());
    assert_eq!(titles(&db), vec!["before".to_string()]);
    add(&mut db, "writable");
    assert_eq!(titles(&db).len(), 2);
}

#[test]
fn restore_aborts_when_safety_backup_fails() {
    use std::os::unix::fs::PermissionsExt;
    let (root, mut db, dir) = setup();
    add(&mut db, "keep");
    let snap = db.snapshot(&dir, false, root.path()).unwrap();
    add(&mut db, "keep2");
    let locked = root.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
    let err = db.restore(&snap.path, &locked, root.path());
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(err.is_err());
    assert_eq!(titles(&db).len(), 2, "the current DB is untouched");
}

#[test]
fn restore_old_snapshot_migrates() {
    let (root, mut db, dir) = setup();
    std::fs::create_dir_all(&dir).unwrap();
    let old = dir.join("track-backup-20250101-000000.db");
    {
        let c = rusqlite::Connection::open(&old).unwrap();
        c.execute_batch(r#"CREATE TABLE "_local_migrations" ("name" TEXT NOT NULL PRIMARY KEY, "appliedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP);"#).unwrap();
        for (name, sql) in &crate::migrate::MIGRATIONS[..7] {
            c.execute_batch(sql).unwrap();
            c.execute(r#"INSERT INTO "_local_migrations"(name) VALUES (?1)"#, [name]).unwrap();
        }
        c.execute(r#"INSERT INTO "User"(id, workStart) VALUES ('u1', 540)"#, []).unwrap();
        c.execute(r#"INSERT INTO "Client"(id, userId, name) VALUES ('c1', 'u1', '旧クライアント')"#, []).unwrap();
    }
    assert!(validate_snapshot(&old).is_ok(), "User instead of Settings is accepted");
    let r = db.restore(&old, &dir, root.path()).unwrap();
    assert!(r.migrated.iter().any(|m| m == "0008_drop_user_id.sql"));
    assert_eq!(db.settings().unwrap().work_start, 540);
    assert_eq!(db.clients().unwrap()[0].name, "旧クライアント");
}

#[test]
fn backup_dir_outside_dev_root_is_rejected() {
    let (root, mut db, _) = setup();
    let outside = tempfile::tempdir().unwrap();
    assert_eq!(db.snapshot(outside.path(), false, root.path()).unwrap_err().code(), "dev_guard");
    let mut cfg = Config::defaults(root.path());
    cfg.export_dir = outside.path().to_path_buf();
    assert_eq!(db.maybe_auto_backup(&cfg, root.path()).unwrap_err().code(), "dev_guard");
    std::fs::write(outside.path().join("track-auto-x.db"), b"x").unwrap();
    assert_eq!(db.restore(&outside.path().join("track-auto-x.db"), &root.path().join("exports"), root.path()).unwrap_err().code(), "dev_guard");
    assert!(outside.path().join("track-auto-x.db").exists(), "nothing pruned or touched outside");
}

#[test]
fn failed_reopen_rolls_back_to_the_safety_backup() {
    let (root, mut db, dir) = setup();
    add(&mut db, "いまのデータ");
    std::fs::create_dir_all(&dir).unwrap();
    // An old snapshot that passes validation but whose 0008 migration fails ("Settings" already exists).
    let bad = dir.join("track-backup-20250101-000000.db");
    {
        let c = rusqlite::Connection::open(&bad).unwrap();
        c.execute_batch(r#"CREATE TABLE "_local_migrations" ("name" TEXT NOT NULL PRIMARY KEY, "appliedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP);"#).unwrap();
        for (name, sql) in &crate::migrate::MIGRATIONS[..7] {
            c.execute_batch(sql).unwrap();
            c.execute(r#"INSERT INTO "_local_migrations"(name) VALUES (?1)"#, [name]).unwrap();
        }
        c.execute_batch(r#"CREATE TABLE "Settings" ("id" TEXT)"#).unwrap();
    }
    assert!(validate_snapshot(&bad).is_ok());
    assert!(db.restore(&bad, &dir, root.path()).is_err());
    assert_eq!(titles(&db), vec!["いまのデータ".to_string()], "rolled back to the data from before the restore");
    add(&mut db, "書ける");
    assert!(db.snapshot(&dir, true, root.path()).is_ok(), "file-backed again");
}

#[test]
fn released_connection_refuses_snapshots() {
    let (root, mut db, dir) = setup();
    db.release_file().unwrap();
    assert_eq!(db.snapshot(&dir, true, root.path()).unwrap_err().code(), "db_unavailable");
    assert!(list_snapshots(&dir).is_empty());
}
