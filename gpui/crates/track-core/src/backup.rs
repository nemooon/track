//! Whole-DB backups and restore (src/server/db/backup.ts). Snapshots are written with
//! `VACUUM INTO`, so each is one consistent file that is safe in a synced folder such as
//! iCloud Drive (unlike the live `track.db` with its `-wal`/`-shm`).
use crate::config::{Config, dev_guard_dir};
use crate::error::{CoreError, Result};
use crate::Db;
use chrono::{DateTime, Local, Utc};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const AUTO_PREFIX: &str = "track-auto-";
const MANUAL_PREFIX: &str = "track-backup-";
const SUFFIX: &str = ".db";
/// Tables a Track DB must have. `User` was renamed to `Settings` in 0008, so either is accepted.
const REQUIRED_TABLES: [&str; 4] = ["Client", "Project", "TimeEntry", "Tag"];
const SETTINGS_TABLES: [&str; 2] = ["Settings", "User"];

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotInfo {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub created_at: DateTime<Utc>,
    pub auto: bool,
}

/// Row counts of the required tables, keyed by table name.
pub type SnapshotCounts = BTreeMap<String, i64>;

#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub safety: SnapshotInfo,
    pub migrated: Vec<String>,
    pub counts: SnapshotCounts,
}

fn info(path: &Path) -> Option<SnapshotInfo> {
    let name = path.file_name()?.to_str()?.to_string();
    let auto = name.starts_with(AUTO_PREFIX);
    if !name.ends_with(SUFFIX) || !(auto || name.starts_with(MANUAL_PREFIX)) {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    Some(SnapshotInfo { name, path: path.to_path_buf(), bytes: meta.len(), created_at: meta.modified().ok()?.into(), auto })
}

/// Snapshots in `dir`, newest first. A missing folder is an empty list.
pub fn list_snapshots(dir: &Path) -> Vec<SnapshotInfo> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<SnapshotInfo> = rd.flatten().filter_map(|e| info(&e.path())).collect();
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.name.cmp(&a.name)));
    out
}

/// Keeps the newest `keep` automatic snapshots; manual ones are never removed.
pub fn prune_snapshots(dir: &Path, keep: i64) -> usize {
    let stale: Vec<SnapshotInfo> = list_snapshots(dir).into_iter().filter(|s| s.auto).skip(keep.max(0) as usize).collect();
    for s in &stale {
        let _ = std::fs::remove_file(&s.path);
    }
    stale.len()
}

/// Checks that `path` really is a Track DB before it replaces the live one.
pub fn validate_snapshot(path: &Path) -> std::result::Result<SnapshotCounts, String> {
    if !path.exists() {
        return Err("ファイルが見つかりません".into());
    }
    let conn = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("SQLite として開けません: {e}"))?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).map_err(|e| format!("SQLite として開けません: {e}"))?;
    if integrity != "ok" {
        return Err(format!("整合性チェックに失敗: {integrity}"));
    }
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .and_then(|mut st| st.query_map([], |r| r.get(0))?.collect())
        .map_err(|e| e.to_string())?;
    let missing: Vec<&str> = REQUIRED_TABLES.iter().copied().filter(|t| !tables.iter().any(|x| x == t)).collect();
    if !missing.is_empty() {
        return Err(format!("Track の DB ではないようです ({} が無い)", missing.join(", ")));
    }
    let Some(settings) = SETTINGS_TABLES.iter().find(|t| tables.iter().any(|x| x == *t)) else {
        return Err("Track の DB ではないようです (Settings も User も無い)".into());
    };
    let mut counts = SnapshotCounts::new();
    for t in REQUIRED_TABLES.iter().chain([settings]) {
        let n: i64 = conn.query_row(&format!(r#"SELECT COUNT(*) FROM "{t}""#), [], |r| r.get(0)).map_err(|e| e.to_string())?;
        counts.insert(t.to_string(), n);
    }
    Ok(counts)
}

impl Db {
    /// Writes `track-{auto|backup}-YYYYMMDD-HHMMSS[-n].db` (local time) into `dir`.
    pub fn snapshot(&self, dir: &Path, auto: bool, dev_root: &Path) -> Result<SnapshotInfo> {
        dev_guard_dir(dir, dev_root)?;
        std::fs::create_dir_all(dir)?;
        let prefix = if auto { AUTO_PREFIX } else { MANUAL_PREFIX };
        let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let mut file = dir.join(format!("{prefix}{stamp}{SUFFIX}"));
        let mut n = 1;
        // VACUUM INTO fails when the target exists.
        while file.exists() {
            file = dir.join(format!("{prefix}{stamp}-{n}{SUFFIX}"));
            n += 1;
        }
        self.conn().execute("VACUUM INTO ?1", [file.to_string_lossy()])?;
        info(&file).ok_or_else(|| CoreError::code_msg("backup_failed", "バックアップを作成できませんでした。"))
    }

    /// Takes an automatic snapshot when the newest one is older than the interval, then prunes.
    pub fn maybe_auto_backup(&self, cfg: &Config, dev_root: &Path) -> Result<Option<SnapshotInfo>> {
        if cfg.backup_interval_hours <= 0 {
            return Ok(None);
        }
        dev_guard_dir(&cfg.export_dir, dev_root)?;
        if let Some(newest) = list_snapshots(&cfg.export_dir).into_iter().find(|s| s.auto) {
            let age = Utc::now() - newest.created_at;
            if age < chrono::Duration::hours(cfg.backup_interval_hours) {
                return Ok(None);
            }
        }
        let s = self.snapshot(&cfg.export_dir, true, dev_root)?;
        prune_snapshots(&cfg.export_dir, cfg.backup_keep);
        Ok(Some(s))
    }

    /// Replaces `track.db` with `path`: validate, save the current DB to `safety_dir` (abort if
    /// that fails), swap the file in, migrate and reopen. `self` is the reopened DB afterwards.
    pub fn restore(&mut self, path: &Path, safety_dir: &Path, dev_root: &Path) -> Result<RestoreResult> {
        self.ensure_writable()?;
        dev_guard_dir(path, dev_root)?;
        dev_guard_dir(safety_dir, dev_root)?;
        let counts = validate_snapshot(path).map_err(|reason| CoreError::with_details("invalid_snapshot", reason.clone(), serde_json::json!({ "reason": reason })))?;
        let safety = self.snapshot(safety_dir, false, dev_root)?;
        let target = self.path.clone();
        let data_dir = target.parent().map(Path::to_path_buf).unwrap_or_default();
        let staged = target.with_extension("db.restore-tmp");
        let _ = std::fs::remove_file(&staged);
        std::fs::copy(path, &staged)?;
        self.release_file()?;
        let swap = (|| -> std::io::Result<()> {
            // A stale WAL would not match the new main file.
            for ext in ["-wal", "-shm"] {
                let side = PathBuf::from(format!("{}{ext}", target.display()));
                if side.exists() {
                    std::fs::remove_file(side)?;
                }
            }
            std::fs::rename(&staged, &target)
        })();
        let _ = std::fs::remove_file(&staged);
        // Reopen whatever is in place now (and run the migrations an old snapshot needs).
        *self = Db::open(&data_dir)?;
        swap?;
        Ok(RestoreResult { safety, migrated: self.applied_now.clone(), counts })
    }
}
