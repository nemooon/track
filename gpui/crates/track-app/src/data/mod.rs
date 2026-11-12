//! Data access for the app: the SQLite DB through track-core, on a dedicated worker thread
//! so the UI thread never blocks. Method names mirror the old HTTP client so views stay the same.
use crate::api::ApiClient;
use std::collections::HashSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use track_core::guard::{data_dir_from_env, dev_guard};
use track_core::model::*;
use track_core::{CoreError, Db};

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
    /// AI generation still goes through the Bun server until track-core grows providers.
    pub ai: ApiClient,
    /// Test hook: operations named here fail with `test_failure`.
    failing: Arc<Mutex<HashSet<&'static str>>>,
}

impl DataClient {
    /// Production: `TRACK_DATA_DIR` (default `~/.track`) behind the development guard.
    pub fn from_env() -> Self {
        Self::open_guarded(&data_dir_from_env(), ApiClient::from_env())
    }

    /// Opens `data_dir` behind the development guard.
    pub fn open_guarded(data_dir: &Path, ai: ApiClient) -> Self {
        let guard = dev_guard(data_dir).err().map(DataError::from);
        Self::start(data_dir.to_path_buf(), guard, ai)
    }

    /// Opens `data_dir` without the development guard (tests use temp dirs).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn open_unguarded(data_dir: &Path, ai: ApiClient) -> Self {
        Self::start(data_dir.to_path_buf(), None, ai)
    }

    fn start(data_dir: PathBuf, preset_error: Option<DataError>, ai: ApiClient) -> Self {
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
        DataClient { tx, data_dir, ai, failing: Arc::default() }
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
                    Ok(db) => f(db).map_err(DataError::from),
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

    pub fn config(&self) -> AppConfig {
        track_core::repo::app_config(&self.data_dir)
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

    pub fn generate_stream(&self, mode: &str, input: String) -> async_channel::Receiver<crate::api::ndjson::AiStreamEvent> {
        self.ai.generate_stream(mode, input)
    }
}
