use crate::app::Page;
use crate::testing::open_app;
use chrono::{Duration, Local};
use gpui::TestAppContext;

#[gpui::test]
fn shortcuts_switch_pages_and_move_period(cx: &mut TestAppContext) {
    let (app, cx) = open_app(cx);
    let start = app.read_with(cx, |a, cx| a.calendar.read(cx).anchor);
    cx.simulate_keystrokes("cmd-]");
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).anchor), start + Duration::days(7));
    cx.simulate_keystrokes("cmd-t");
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).anchor), Local::now().date_naive());
    cx.simulate_keystrokes("cmd-2");
    assert_eq!(app.read_with(cx, |a, _| a.page), Page::Reports);
    cx.simulate_keystrokes("cmd-[");
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).anchor), Local::now().date_naive() - Duration::days(7));
    cx.simulate_keystrokes("cmd-1");
    assert_eq!(app.read_with(cx, |a, _| a.page), Page::Calendar);
}

#[gpui::test]
fn zoom_shortcuts_clamp_and_persist(cx: &mut TestAppContext) {
    let (app, cx) = open_app(cx);
    cx.simulate_keystrokes("cmd-= cmd-= cmd-=");
    assert_eq!(app.read_with(cx, |a, cx| a.state.read(cx).persist.zoom_index), 2);
    cx.simulate_keystrokes("cmd-- cmd-- cmd-- cmd--");
    assert_eq!(app.read_with(cx, |a, cx| a.state.read(cx).persist.zoom_index), 0);
}

#[gpui::test]
fn date_picker_selects_week(cx: &mut TestAppContext) {
    use chrono::Datelike;
    let (app, cx) = open_app(cx);
    let label = cx.debug_bounds("nav-label").expect("label");
    cx.simulate_click(label.center(), gpui::Modifiers::none());
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).nav_open()));
    let popup = cx.debug_bounds("date-picker").expect("picker");
    assert!(popup.origin.y >= label.origin.y + label.size.height, "picker opens below the label: {popup:?} vs {label:?}");
    assert!((popup.origin.x - label.origin.x).abs() < gpui::px(80.), "picker is horizontally aligned with the navigator");
    // Calendar default (稼働日) uses the week picker; pick the week containing the 1st of this month.
    let first = Local::now().date_naive().with_day(1).unwrap();
    let ws = crate::core::time::week_start(first);
    let sel: &'static str = Box::leak(format!("pick-week-{ws}").into_boxed_str());
    let row = cx.debug_bounds(sel).expect("week row");
    cx.simulate_click(row.center(), gpui::Modifiers::none());
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).anchor), ws);
    assert!(!app.read_with(cx, |a, cx| a.calendar.read(cx).nav_open()));
}

#[gpui::test]
fn server_change_events_reload_both_screens(cx: &mut TestAppContext) {
    let (base, log) = crate::testing::stub_server(|_, path| match path {
        "/api/events" => (200, "event: entries.changed\ndata: {\"action\":\"created\"}\n\n".into()),
        "/api/entries" => (200, "[]".into()),
        "/api/reports" => (200, r#"{"rows":[],"totalMinutes":0,"range":{"from":"2026-11-07T15:00:00Z","to":"2026-11-14T15:00:00Z"}}"#.into()),
        _ => (404, "{}".into()),
    });
    let (_app, cx) = crate::testing::open_app_with(cx, base);
    cx.simulate_keystrokes("cmd-2 cmd-1");
    let count = |path: &str| log.lock().unwrap().iter().filter(|r| r.path.starts_with(path) && r.path[path.len()..].starts_with('?')).count();
    for _ in 0..150 {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // initial load + at least one event-driven reload (the stub stream closes, so it reconnects and fires again)
    assert!(count("/api/entries") >= 2, "entries fetched {} times", count("/api/entries"));
    assert!(count("/api/reports") >= 2, "reports fetched {} times", count("/api/reports"));
}

fn reference_stub(db: &'static str) -> String {
    let (base, _) = crate::testing::stub_server(move |_, path| match path {
        "/health" => (200, format!(r#"{{"ok":true,"db":"{db}"}}"#)),
        "/api/projects" | "/api/clients" | "/api/tags" => (200, "[]".into()),
        "/api/settings" => (200, r#"{"workStart":600,"workEnd":1110,"workDays":[1,2,3,4,5]}"#.into()),
        "/api/config" => (200, r#"{"aiProvider":"codex"}"#.into()),
        "/api/entries" => (200, "[]".into()),
        _ => (404, "{}".into()),
    });
    base
}

fn settle(cx: &mut gpui::VisualTestContext) {
    for _ in 0..80 {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[gpui::test]
fn refuses_to_run_against_the_real_track_db(cx: &mut TestAppContext) {
    let (app, cx) = crate::testing::open_app_loading(cx, reference_stub("/Users/someone/.track/track.db"));
    settle(cx);
    let (data, err) = app.read_with(cx, |a, cx| (a.state.read(cx).data.is_some(), a.state.read(cx).error.clone()));
    assert!(!data, "must not load reference data from the real DB");
    assert!(err.unwrap_or_default().contains("/Users/someone/.track/track.db"));
}

#[gpui::test]
fn loads_when_server_uses_the_copy_db(cx: &mut TestAppContext) {
    let (app, cx) = crate::testing::open_app_loading(cx, reference_stub("/Users/someone/.track-gpui/track.db"));
    settle(cx);
    assert!(app.read_with(cx, |a, cx| a.state.read(cx).data.is_some()));
}

#[gpui::test]
fn calendar_shows_error_when_entries_fail(cx: &mut TestAppContext) {
    let (app, cx) = crate::testing::open_app_with(cx, "http://127.0.0.1:9/api".into());
    settle(cx);
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).load_error.is_some()));
    assert!(cx.debug_bounds("calendar-retry").is_some(), "retry button shown");
}

#[gpui::test]
fn reports_show_error_instead_of_loading_forever(cx: &mut TestAppContext) {
    let (app, cx) = crate::testing::open_app_with(cx, "http://127.0.0.1:9/api".into());
    cx.simulate_keystrokes("cmd-2");
    settle(cx);
    assert!(app.read_with(cx, |a, cx| a.reports.read(cx).error.is_some()));
    assert!(cx.debug_bounds("reports-retry").is_some(), "retry button shown");
}

#[gpui::test]
fn failed_child_rows_do_not_stay_loading(cx: &mut TestAppContext) {
    let (base, _) = crate::testing::stub_server(|_, path| match path {
        "/api/reports" => (200, r##"{"rows":[{"key":"p1","label":"A · B","color":"#3b82f6","totalMinutes":60}],"totalMinutes":60,"range":{"from":"2026-11-07T15:00:00Z","to":"2026-11-14T15:00:00Z"}}"##.into()),
        "/api/entries" => (200, "[]".into()),
        _ => (500, r#"{"error":"boom"}"#.into()),
    });
    let (app, cx) = crate::testing::open_app_with(cx, base);
    cx.simulate_keystrokes("cmd-2");
    settle(cx);
    let b = cx.debug_bounds("row-p1").expect("row");
    cx.simulate_click(b.center(), gpui::Modifiers::none());
    settle(cx);
    let state = app.read_with(cx, |a, cx| a.reports.read(cx).children.get("project:p1").cloned());
    assert!(matches!(state, Some(crate::reports::Children::Failed(_))), "{state:?}");
}

#[gpui::test]
fn reconnect_event_retries_startup_load(cx: &mut TestAppContext) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let health_calls = std::sync::Arc::new(AtomicUsize::new(0));
    let hc = health_calls.clone();
    let (base, _) = crate::testing::stub_server(move |_, path| match path {
        // first startup check fails (server "not up yet"), later ones succeed
        "/health" if hc.fetch_add(1, Ordering::SeqCst) == 0 => (503, "{}".into()),
        "/health" => (200, r#"{"ok":true,"db":"/x/.track-gpui/track.db"}"#.into()),
        "/api/events" => (200, "event: track.connected\ndata: {}\n\n".into()),
        "/api/projects" | "/api/clients" | "/api/tags" | "/api/entries" => (200, "[]".into()),
        "/api/settings" => (200, r#"{"workStart":600,"workEnd":1110,"workDays":[1,2,3,4,5]}"#.into()),
        "/api/config" => (200, r#"{"aiProvider":"codex"}"#.into()),
        _ => (404, "{}".into()),
    });
    let (app, cx) = crate::testing::open_app_loading(cx, base);
    for _ in 0..200 {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(app.read_with(cx, |a, cx| a.state.read(cx).data.is_some()), "track.connected should retry the startup load");
}
