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
        config: AppConfig { ai_provider: "codex".into() },
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
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::actions::bind_keys(cx);
    });
    let dir = tempfile::tempdir().unwrap().keep();
    let (view, vcx) = cx.add_window_view(|window, cx| {
        let state = cx.new(|_| {
            let mut s = AppState::with_path(ApiClient::new("http://127.0.0.1:9/api".into()), dir.join("p.json"));
            s.data = Some(fake_data());
            s
        });
        TrackApp::with_state(state, window, cx)
    });
    (view, vcx)
}
