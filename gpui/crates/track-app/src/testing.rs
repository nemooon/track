//! Headless UI test helpers (GPUI TestAppContext) over a temporary SQLite DB.
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
pub fn open_app_db(cx: &mut TestAppContext, seed: impl FnOnce(&mut Db)) -> (Entity<TrackApp>, &mut VisualTestContext, Fixture) {
    let dir = tempfile::tempdir().unwrap().keep();
    {
        let mut db = Db::open(&dir).unwrap();
        seed_reference(&mut db);
        seed(&mut db);
    }
    let data = DataClient::open_unguarded(&dir);
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
    let (app, vcx, _f) = open_app_db(cx, |_| {});
    (app, vcx)
}

fn open_with(cx: &mut TestAppContext, data: DataClient, persist: PathBuf, preloaded: Option<AppData>) -> (Entity<TrackApp>, &mut VisualTestContext) {
    // DB work runs on its own thread, outside the deterministic scheduler.
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::ui::theme::apply_theme(false, cx);
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
        view.update(cx, |a, cx| a.calendar.update(cx, |c, cx| c.pin_daytime_scroll(cx)));
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

/// Points the app's AI at a custom command that records its stdin to `<dir>/ai-input.txt`
/// and answers `text`.
pub fn fake_ai(dir: &Path, text: &str) {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("fake-ai.sh");
    std::fs::write(dir.join("ai-answer.txt"), text).unwrap();
    std::fs::write(&script, "#!/bin/sh\nd=$(dirname \"$0\")\ncat > \"$d/ai-input.txt\"\ncat \"$d/ai-answer.txt\"\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = serde_json::json!({ "aiProvider": "custom-command", "aiCommandExecutable": script });
    std::fs::write(dir.join(track_core::config::FILE), config.to_string()).unwrap();
}

/// A day the default calendar (稼働日) shows this week: Monday of the current Sunday-start week.
/// `Local::now()` itself is hidden on Saturdays and Sundays.
pub fn visible_weekday() -> NaiveDate {
    crate::core::time::week_start(chrono::Local::now().date_naive()) + chrono::Duration::days(1)
}
