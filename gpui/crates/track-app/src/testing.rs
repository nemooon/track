//! Headless UI test helpers (GPUI TestAppContext).
use crate::api::*;
use crate::app::{AppData, AppState, TrackApp};
use chrono::{Local, TimeZone};
use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};

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

/// Opens a TrackApp window whose API points at an unused port, with fake reference data loaded.
pub fn open_app(cx: &mut TestAppContext) -> (Entity<TrackApp>, &mut VisualTestContext) {
    open_app_with(cx, "http://127.0.0.1:9/api".into())
}

/// A local server that accepts connections and never answers (requests stay pending).
pub fn hanging_server() -> String {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut keep = Vec::new();
        for s in l.incoming().flatten() {
            keep.push(s);
        }
    });
    format!("http://127.0.0.1:{port}/api")
}

pub fn open_app_with(cx: &mut TestAppContext, base: String) -> (Entity<TrackApp>, &mut VisualTestContext) {
    open_app_inner(cx, base, true)
}

/// Like `open_app_with`, but runs the real startup load (`AppState::load`) against `base`.
pub fn open_app_loading(cx: &mut TestAppContext, base: String) -> (Entity<TrackApp>, &mut VisualTestContext) {
    open_app_inner(cx, base, false)
}

fn open_app_inner(cx: &mut TestAppContext, base: String, prefilled: bool) -> (Entity<TrackApp>, &mut VisualTestContext) {
    // Requests run on the API client's tokio runtime, outside the deterministic scheduler.
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::actions::bind_keys(cx);
    });
    let dir = tempfile::tempdir().unwrap().keep();
    let slot: std::rc::Rc<std::cell::RefCell<Option<Entity<TrackApp>>>> = Default::default();
    let slot2 = slot.clone();
    let (_root, vcx) = cx.add_window_view(move |window, cx| {
        let base = base.clone();
        let state = cx.new(|_| {
            let mut s = AppState::with_path(ApiClient::new(base.clone()), dir.join("p.json"));
            if prefilled {
                s.data = Some(fake_data());
            }
            s
        });
        if !prefilled {
            state.update(cx, |s, cx| s.load(cx));
        }
        let view = cx.new(|cx| TrackApp::with_state(state, window, cx));
        *slot2.borrow_mut() = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = slot.borrow().clone().unwrap();
    (view, vcx)
}

#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub body: String,
}

/// Minimal HTTP stub: answers each request with `route(method, path)` and records it.
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
