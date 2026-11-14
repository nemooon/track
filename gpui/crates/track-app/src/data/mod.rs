//! Data access for the app: the SQLite DB through track-core, on a dedicated worker thread
//! so the UI thread never blocks. Method names mirror the old HTTP client so views stay the same.
use std::collections::HashSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use track_core::guard::{data_dir_from_env, dev_guard};
use track_core::model::*;
use track_core::ai::{AiMode, provider_from_config};
use track_core::{CoreError, Db};

/// What an AI generation reports while it runs.
#[derive(Debug, Clone, PartialEq)]
pub enum AiStreamEvent {
    Progress { kind: String, message: String },
    Result { text: String, provider: String },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataError {
    pub code: String,
    pub message: String,
}

impl From<CoreError> for DataError {
    fn from(e: CoreError) -> Self {
        DataError { code: e.code().to_string(), message: e.to_string() }
    }
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub type DataResult<T> = Result<T, DataError>;

type Job = Box<dyn FnOnce(Result<&mut Db, &DataError>) + Send>;

#[derive(Clone)]
pub struct DataClient {
    tx: mpsc::Sender<Job>,
    pub data_dir: PathBuf,
    /// Backups may only be written and restored under this folder while the dev guard is on
    /// (`~/.track-gpui`; tests use their temp data dir).
    pub dev_root: PathBuf,
    /// Test hook: operations named here fail with `test_failure`.
    failing: Arc<Mutex<HashSet<&'static str>>>,
}

impl DataClient {
    /// Production: `TRACK_DATA_DIR` (default `~/.track`) behind the development guard.
    pub fn from_env() -> Self {
        Self::open_guarded(&data_dir_from_env())
    }

    /// Opens `data_dir` behind the development guard.
    pub fn open_guarded(data_dir: &Path) -> Self {
        let guard = dev_guard(data_dir).err().map(DataError::from);
        let root = track_core::guard::allowed_root();
        // Seed gpui-config.json only where the guard lets us write (never the real ~/.track
        // while in development).
        if guard.is_none() {
            let _ = track_core::config::migrate_in(data_dir, &root);
        }
        Self::start(data_dir.to_path_buf(), root, guard)
    }

    /// Opens `data_dir` without the development guard (tests use temp dirs).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn open_unguarded(data_dir: &Path) -> Self {
        let _ = track_core::config::migrate_in(data_dir, data_dir);
        Self::start(data_dir.to_path_buf(), data_dir.to_path_buf(), None)
    }

    fn start(data_dir: PathBuf, dev_root: PathBuf, preset_error: Option<DataError>) -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        let dir = data_dir.clone();
        std::thread::Builder::new()
            .name("track-db".into())
            .spawn(move || {
                let mut db: Result<Db, DataError> = match preset_error {
                    Some(e) => Err(e),
                    None => Db::open(&dir).map_err(DataError::from),
                };
                for job in rx {
                    match &mut db {
                        Ok(db) => job(Ok(db)),
                        Err(e) => job(Err(e)),
                    }
                }
            })
            .expect("db thread");
        DataClient { tx, data_dir, dev_root, failing: Arc::default() }
    }

    /// Runs `f` on the DB thread. A closed DB (open failure, guard) answers every call with that error.
    pub fn call<T: Send + 'static>(
        &self,
        op: &'static str,
        f: impl FnOnce(&mut Db) -> Result<T, CoreError> + Send + 'static,
    ) -> impl Future<Output = DataResult<T>> + Send + 'static {
        let (tx, rx) = async_channel::bounded(1);
        let fail = self.failing.lock().unwrap().contains(op);
        let job: Job = Box::new(move |db| {
            let result = if fail {
                Err(DataError { code: "test_failure".into(), message: format!("{op} failed (test)") })
            } else {
                match db {
                    // A bug in one operation must not take the DB thread (and every later call) down with it.
                    Ok(db) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(db)))
                        .unwrap_or_else(|_| Err(CoreError::code_msg("internal_error", format!("{op} で内部エラーが発生しました。"))))
                        .map_err(DataError::from),
                    Err(e) => Err(e.clone()),
                }
            };
            let _ = tx.send_blocking(result);
        });
        let sent = self.tx.send(job).is_ok();
        async move {
            if !sent {
                return Err(DataError { code: "closed".into(), message: "DB スレッドが終了しています。".into() });
            }
            rx.recv().await.unwrap_or_else(|_| Err(DataError { code: "closed".into(), message: "DB スレッドが終了しています。".into() }))
        }
    }

    /// Tests: save settings synchronously, bypassing the DB thread.
    #[cfg(test)]
    pub fn save_config_blocking(&self, patch: track_core::config::ConfigPatch) {
        track_core::config::save_config_in(&self.data_dir, patch, &self.dev_root).unwrap();
    }

    #[cfg(test)]
    pub fn fail_ops(&self, ops: &[&'static str]) {
        self.failing.lock().unwrap().extend(ops.iter().copied());
    }

    pub fn entries(&self, from: chrono::DateTime<chrono::Utc>, to: chrono::DateTime<chrono::Utc>) -> impl Future<Output = DataResult<Vec<TimeEntry>>> + Send + 'static {
        self.call("entries", move |db| db.entries(from, to))
    }

    /// `None` = entries without a project (like `?projectId=none`).
    pub fn entry_titles(&self, project_id: Option<&str>) -> impl Future<Output = DataResult<Vec<String>>> + Send + 'static {
        let p = project_id.unwrap_or("none").to_string();
        self.call("entry_titles", move |db| db.entry_titles(Some(&p)))
    }

    pub fn create_entry(&self, body: NewEntry) -> impl Future<Output = DataResult<TimeEntry>> + Send + 'static {
        self.call("create_entry", move |db| db.create_entry(body))
    }

    pub fn update_entry(&self, id: &str, patch: EntryPatch) -> impl Future<Output = DataResult<TimeEntry>> + Send + 'static {
        let id = id.to_string();
        self.call("update_entry", move |db| db.update_entry(&id, patch))
    }

    pub fn delete_entry(&self, id: &str) -> impl Future<Output = DataResult<()>> + Send + 'static {
        let id = id.to_string();
        self.call("delete_entry", move |db| db.delete_entry(&id))
    }

    pub fn projects(&self) -> impl Future<Output = DataResult<Vec<Project>>> + Send + 'static {
        self.call("projects", |db| db.projects(false))
    }

    pub fn clients(&self) -> impl Future<Output = DataResult<Vec<Client>>> + Send + 'static {
        self.call("clients", |db| db.clients())
    }

    pub fn tags(&self) -> impl Future<Output = DataResult<Vec<Tag>>> + Send + 'static {
        self.call("tags", |db| db.tags())
    }

    pub fn settings(&self) -> impl Future<Output = DataResult<Settings>> + Send + 'static {
        self.call("settings", |db| db.settings())
    }

    pub fn latest_operation_id(&self) -> impl Future<Output = DataResult<i64>> + Send + 'static {
        self.call("latest_operation_id", |db| db.latest_operation_id())
    }

    pub fn operations_after(&self, id: i64) -> impl Future<Output = DataResult<Vec<track_core::ops::Operation>>> + Send + 'static {
        self.call("operations_after", move |db| db.operations_after(id))
    }

    /// Which of `ids` can still be undone (not undone, not pruned).
    pub fn pending_operations(&self, ids: Vec<i64>) -> impl Future<Output = DataResult<Vec<i64>>> + Send + 'static {
        self.call("pending_operations", move |db| {
            let mut out = Vec::new();
            for id in ids {
                if db.operation(id)?.is_some_and(|o| o.undone_at.is_none()) {
                    out.push(id);
                }
            }
            Ok(out)
        })
    }

    /// Undoes `ids` in the given order (newest first), stopping at the first that fails.
    /// Returns the ids that are no longer pending (undone now or already) and that failure.
    pub fn undo_operations(&self, ids: Vec<i64>) -> impl Future<Output = DataResult<(Vec<i64>, Option<DataError>)>> + Send + 'static {
        self.call("undo_operations", move |db| {
            let mut done = Vec::new();
            for id in ids {
                match db.undo_operation(Some(id)) {
                    Ok(_) => done.push(id),
                    Err(e) if matches!(e.code(), "already_undone" | "not_found") => done.push(id),
                    Err(e) => return Ok((done, Some(DataError::from(e)))),
                }
            }
            Ok((done, None))
        })
    }

    pub fn projects_all(&self) -> impl Future<Output = DataResult<Vec<Project>>> + Send + 'static {
        self.call("projects_all", |db| db.projects(true))
    }

    pub fn create_client(&self, name: String) -> impl Future<Output = DataResult<String>> + Send + 'static {
        self.call("create_client", move |db| db.create_client(&name))
    }

    pub fn update_client(&self, id: String, patch: ClientPatch) -> impl Future<Output = DataResult<Client>> + Send + 'static {
        self.call("update_client", move |db| db.update_client(&id, patch))
    }

    pub fn create_project(&self, client_id: String, name: String, color: String, tag_ids: Vec<String>) -> impl Future<Output = DataResult<String>> + Send + 'static {
        self.call("create_project", move |db| db.create_project_with_tags(&client_id, &name, &color, &tag_ids))
    }

    pub fn update_project(&self, id: String, patch: ProjectPatch) -> impl Future<Output = DataResult<Project>> + Send + 'static {
        self.call("update_project", move |db| db.update_project(&id, patch))
    }

    pub fn create_tag(&self, name: String, color: String) -> impl Future<Output = DataResult<String>> + Send + 'static {
        self.call("create_tag", move |db| db.create_tag(&name, &color))
    }

    pub fn update_tag(&self, id: String, patch: TagPatch) -> impl Future<Output = DataResult<Tag>> + Send + 'static {
        self.call("update_tag", move |db| db.update_tag(&id, patch))
    }

    pub fn delete_tag(&self, id: String) -> impl Future<Output = DataResult<()>> + Send + 'static {
        self.call("delete_tag", move |db| db.delete_tag(&id))
    }

    pub fn update_settings(&self, patch: SettingsPatch) -> impl Future<Output = DataResult<Settings>> + Send + 'static {
        self.call("update_settings", move |db| db.update_settings(patch))
    }

    /// Saves `gpui-config.json` (off the UI thread; the DB is not involved).
    pub fn save_config(&self, patch: track_core::config::ConfigPatch) -> impl Future<Output = DataResult<track_core::config::Config>> + Send + 'static {
        let (dir, root) = (self.data_dir.clone(), self.dev_root.clone());
        self.call("save_config", move |_| track_core::config::save_config_in(&dir, patch, &root))
    }

    /// Snapshots in the configured backup folder, newest first.
    pub fn snapshots(&self) -> Vec<track_core::backup::SnapshotInfo> {
        track_core::backup::list_snapshots(&self.full_config().export_dir)
    }

    pub fn backup_now(&self) -> impl Future<Output = DataResult<track_core::backup::SnapshotInfo>> + Send + 'static {
        let (dir, root) = (self.full_config().export_dir, self.dev_root.clone());
        self.call("backup_now", move |db| db.snapshot(&dir, false, &root))
    }

    pub fn maybe_auto_backup(&self) -> impl Future<Output = DataResult<Option<track_core::backup::SnapshotInfo>>> + Send + 'static {
        let (cfg, root) = (self.full_config(), self.dev_root.clone());
        self.call("maybe_auto_backup", move |db| db.maybe_auto_backup(&cfg, &root))
    }

    /// Replaces the DB with `path` on the DB thread (the current DB is saved first).
    pub fn restore(&self, path: PathBuf) -> impl Future<Output = DataResult<track_core::backup::RestoreResult>> + Send + 'static {
        let (dir, root) = (self.full_config().export_dir, self.dev_root.clone());
        self.call("restore", move |db| db.restore(&path, &dir, &root))
    }

    pub fn full_config(&self) -> track_core::config::Config {
        track_core::config::load_config(&self.data_dir)
    }

    pub fn config(&self) -> AppConfig {
        track_core::config::load_config(&self.data_dir).app_config()
    }

    pub fn report(&self, q: &ReportQuery) -> impl Future<Output = DataResult<ReportResponse>> + Send + 'static {
        let q = q.clone();
        self.call("report", move |db| db.report(&q))
    }

    pub fn weekly_copy(&self, q: &ReportQuery) -> impl Future<Output = DataResult<String>> + Send + 'static {
        let q = q.clone();
        self.call("weekly_copy", move |db| db.weekly_copy(&q))
    }

    pub fn report_entries(&self, q: &ReportQuery) -> impl Future<Output = DataResult<ReportEntriesResponse>> + Send + 'static {
        let q = q.clone();
        self.call("report_entries", move |db| db.report_entries(&q))
    }

    pub fn data_version(&self) -> impl Future<Output = DataResult<i64>> + Send + 'static {
        self.call("data_version", |db| db.data_version())
    }

    /// Runs the configured AI provider on its own thread, streaming progress then one result or error.
    pub fn generate_stream(&self, mode: AiMode, input: String) -> async_channel::Receiver<AiStreamEvent> {
        let (tx, rx) = async_channel::unbounded();
        let config = self.config();
        std::thread::Builder::new()
            .name("track-ai".into())
            .spawn(move || {
                let provider = provider_from_config(&config);
                let result = provider.generate(mode, &input, &mut |p| {
                    let _ = tx.send_blocking(AiStreamEvent::Progress { kind: p.kind.to_string(), message: p.message });
                });
                let _ = tx.send_blocking(match result {
                    Ok(text) => AiStreamEvent::Result { text, provider: provider.id().to_string() },
                    Err(e) => AiStreamEvent::Error { message: e.to_string() },
                });
            })
            .expect("ai thread");
        rx
    }
}
