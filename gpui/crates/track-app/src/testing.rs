//! Headless UI test helpers (GPUI TestAppContext) over a temporary SQLite DB.
use crate::api::ApiClient;
use crate::app::{AppData, AppState, TrackApp};
use crate::data::DataClient;
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};
use std::path::{Path, PathBuf};
use track_core::model::*;
use track_core::{Db, time};

pub fn fake_data() -> AppData {
    let client = Client { id: "c1".into(), name: "Acme".into(), archived: false };
    let tag = Tag { id: "t1".into(), name: "定例".into(), color: "#22c55e".into() };
    AppData {
        projects: vec![
            Project { id: "p1".into(), client_id: "c1".into(), name: "Alpha".into(), color: "#3b82f6".into(), archived: false, client: client.clone(), tags: vec![TagLink { tag_id: "t1".into(), tag: tag.clone() }] },
            Project { id: "p2".into(), client_id: "c1".into(), name: "Beta".into(), color: "#ef4444".into(), archived: false, client: client.clone(), tags: vec![] },
        ],
        clients: vec![client],
        tags: vec![tag],
        settings: Settings { work_start: 600, work_end: 1110, work_days: vec![1, 2, 3, 4, 5], weekly_report_template: "{{期間}} {{合計時間}}".into(), report_copy_formats: vec![] },
        config: AppConfig { ai_provider: "codex".into(), ..Default::default() },
    }
}

pub fn entry(id: &str, day: u32, (sh, sm): (u32, u32), (eh, em): (u32, u32), project: Option<&Project>) -> TimeEntry {
    let t = |h, m| Local.with_ymd_and_hms(2026, 11, day, h, m, 0).unwrap().to_utc();
    TimeEntry {
        id: id.into(), project_id: project.map(|p| p.id.clone()), start: t(sh, sm), end: t(eh, em), title: Some(format!("作業{id}")),
        note: None, project: project.cloned(), tags: vec![], external_event_id: None, external_event_source: None, break_minutes: 0,
    }
}

pub fn local(day: NaiveDate, h: u32, m: u32) -> DateTime<Utc> {
    Local.from_local_datetime(&day.and_hms_opt(h, m, 0).unwrap()).unwrap().to_utc()
}

/// The same reference data as `fake_data()`, with fixed ids: client c1 "Acme",
/// project p1 "Alpha" (project tag t1 "定例"), project p2 "Beta".
pub fn seed_reference(db: &mut Db) {
    let now = time::now_db();
    let c = db.conn();
    c.execute(r#"INSERT INTO "Client"(id, name, createdAt) VALUES ('c1', 'Acme', ?1)"#, [&now]).unwrap();
    c.execute(r##"INSERT INTO "Project"(id, clientId, name, color, createdAt) VALUES ('p1', 'c1', 'Alpha', '#3b82f6', ?1)"##, [&now]).unwrap();
    c.execute(r##"INSERT INTO "Project"(id, clientId, name, color, createdAt) VALUES ('p2', 'c1', 'Beta', '#ef4444', ?1)"##, [&now]).unwrap();
    c.execute(r##"INSERT INTO "Tag"(id, name, color, createdAt) VALUES ('t1', '定例', '#22c55e', ?1)"##, [&now]).unwrap();
    c.execute(r#"INSERT INTO "TagOnProject"(id, tagId, projectId) VALUES ('tp1', 't1', 'p1')"#, []).unwrap();
    c.execute(r#"UPDATE "Settings" SET weeklyReportTemplate = '{{期間}} {{合計時間}}'"#, []).unwrap();
}

/// Inserts an entry with a fixed id.
pub fn insert_entry(db: &Db, id: &str, start: DateTime<Utc>, end: DateTime<Utc>, project: Option<&str>, title: &str) {
    let now = time::now_db();
    db.conn()
        .execute(
            r#"INSERT INTO "TimeEntry"(id, projectId, start, "end", title, breakMinutes, createdAt, updatedAt) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)"#,
            rusqlite::params![id, project, time::to_db(start), time::to_db(end), title, now],
        )
        .unwrap();
}

pub struct Fixture {
    pub dir: PathBuf,
    pub data: DataClient,
}

impl Fixture {
    /// A second connection to inspect what the app wrote.
    pub fn db(&self) -> Db {
        Db::open(&self.dir).unwrap()
    }
}

fn read_app_data(dir: &Path) -> AppData {
    let db = Db::open(dir).unwrap();
    AppData {
        projects: db.projects(false).unwrap(),
        clients: db.clients().unwrap(),
        tags: db.tags().unwrap(),
        settings: db.settings().unwrap(),
        config: AppConfig { ai_provider: "codex".into(), ..Default::default() },
    }
}

/// Opens a TrackApp over a fresh temp DB with the reference data (`seed_reference`) plus `seed`.
/// AI generation goes to `ai_base` (a stub server or an unused port).
pub fn open_app_db<'a>(cx: &'a mut TestAppContext, seed: impl FnOnce(&mut Db), ai_base: &str) -> (Entity<TrackApp>, &'a mut VisualTestContext, Fixture) {
    let dir = tempfile::tempdir().unwrap().keep();
    {
        let mut db = Db::open(&dir).unwrap();
        seed_reference(&mut db);
        seed(&mut db);
    }
    let data = DataClient::open_unguarded(&dir, ApiClient::new(ai_base.to_string()));
    let app_data = read_app_data(&dir);
    let (app, vcx) = open_with(cx, data.clone(), dir.join("p.json"), Some(app_data));
    (app, vcx, Fixture { dir, data })
}

/// Opens a TrackApp that runs the real startup load (`AppState::load`) through `data`.
pub fn open_app_loading(cx: &mut TestAppContext, data: DataClient) -> (Entity<TrackApp>, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap().keep();
    open_with(cx, data, dir.join("p.json"), None)
}

/// Default: empty reference data, AI pointed at an unused port.
pub fn open_app(cx: &mut TestAppContext) -> (Entity<TrackApp>, &mut VisualTestContext) {
    let (app, vcx, _f) = open_app_db(cx, |_| {}, "http://127.0.0.1:9/api");
    (app, vcx)
}

fn open_with(cx: &mut TestAppContext, data: DataClient, persist: PathBuf, preloaded: Option<AppData>) -> (Entity<TrackApp>, &mut VisualTestContext) {
    // DB work runs on its own thread, outside the deterministic scheduler.
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::actions::bind_keys(cx);
    });
    let slot: std::rc::Rc<std::cell::RefCell<Option<Entity<TrackApp>>>> = Default::default();
    let slot2 = slot.clone();
    let (_root, vcx) = cx.add_window_view(move |window, cx| {
        let loaded = preloaded.is_some();
        let state = cx.new(|_| {
            let mut s = AppState::with_path(data.clone(), persist.clone());
            s.data = preloaded.clone();
            s
        });
        if !loaded {
            state.update(cx, |s, cx| s.load(cx));
        }
        let view = cx.new(|cx| TrackApp::with_state(state, window, cx));
        *slot2.borrow_mut() = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = slot.borrow().clone().unwrap();
    (view, vcx)
}

/// Let DB-thread and network results land.
pub fn wait_io(cx: &mut VisualTestContext) {
    for _ in 0..60 {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // method kept for debugging stub traffic
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub body: String,
}

/// Minimal HTTP stub (used for the AI stream until track-core has providers).
pub fn stub_server(route: impl Fn(&str, &str) -> (u16, String) + Send + Sync + 'static) -> (String, std::sync::Arc<std::sync::Mutex<Vec<Recorded>>>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let log: std::sync::Arc<std::sync::Mutex<Vec<Recorded>>> = Default::default();
    let log2 = log.clone();
    std::thread::spawn(move || {
        for stream in l.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            let mut parts = line.split_whitespace();
            let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
            let mut len = 0usize;
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).is_err() || h == "\r\n" || h.is_empty() {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; len];
            let _ = reader.read_exact(&mut body);
            let path_only = path.split('?').next().unwrap_or("").to_string();
            log2.lock().unwrap().push(Recorded { method: method.clone(), path: path.clone(), body: String::from_utf8_lossy(&body).into() });
            let (status, resp) = route(&method, &path_only);
            let mut s = stream;
            let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp}", resp.len());
        }
    });
    (format!("http://127.0.0.1:{port}/api"), log)
}
