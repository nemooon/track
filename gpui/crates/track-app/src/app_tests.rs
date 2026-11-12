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

fn settle(cx: &mut gpui::VisualTestContext) {
    crate::testing::wait_io(cx);
}

#[gpui::test]
fn refuses_to_run_outside_the_copy_db(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let data = crate::data::DataClient::open_guarded(dir.path(), crate::api::ApiClient::new("http://127.0.0.1:9/api".into()));
    let (app, cx) = crate::testing::open_app_loading(cx, data);
    settle(cx);
    let (loaded, err) = app.read_with(cx, |a, cx| (a.state.read(cx).data.is_some(), a.state.read(cx).error.clone()));
    assert!(!loaded);
    assert!(err.unwrap_or_default().contains(".track-gpui"));
    assert!(!dir.path().join("track.db").exists(), "the guard must stop before creating a DB");
}

#[gpui::test]
fn loads_reference_data_from_the_db(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = track_core::Db::open(dir.path()).unwrap();
        crate::testing::seed_reference(&mut db);
    }
    let data = crate::data::DataClient::open_unguarded(dir.path(), crate::api::ApiClient::new("http://127.0.0.1:9/api".into()));
    let (app, cx) = crate::testing::open_app_loading(cx, data);
    settle(cx);
    let names = app.read_with(cx, |a, cx| a.state.read(cx).data.as_ref().map(|d| d.projects.iter().map(|p| p.name.clone()).collect::<Vec<_>>()));
    assert_eq!(names, Some(vec!["Alpha".to_string(), "Beta".to_string()]));
}

#[gpui::test]
fn calendar_shows_error_when_entries_fail(cx: &mut TestAppContext) {
    let (app, cx, f) = crate::testing::open_app_db(cx, |_| {}, "http://127.0.0.1:9/api");
    f.data.fail_ops(&["entries"]);
    cx.simulate_keystrokes("cmd-]");
    settle(cx);
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).load_error.is_some()));
    assert!(cx.debug_bounds("calendar-retry").is_some(), "retry button shown");
}

#[gpui::test]
fn reports_show_error_instead_of_loading_forever(cx: &mut TestAppContext) {
    let (app, cx, f) = crate::testing::open_app_db(cx, |_| {}, "http://127.0.0.1:9/api");
    f.data.fail_ops(&["report"]);
    cx.simulate_keystrokes("cmd-2");
    settle(cx);
    assert!(app.read_with(cx, |a, cx| a.reports.read(cx).error.is_some()));
    assert!(cx.debug_bounds("reports-retry").is_some(), "retry button shown");
}

#[gpui::test]
fn failed_child_rows_do_not_stay_loading(cx: &mut TestAppContext) {
    let today = Local::now().date_naive();
    let (app, cx, f) = crate::testing::open_app_db(
        cx,
        |db| crate::testing::insert_entry(db, "e1", crate::testing::local(today, 0, 0), crate::testing::local(today, 1, 0), Some("p1"), "x"),
        "http://127.0.0.1:9/api",
    );
    f.data.fail_ops(&["report_entries"]);
    cx.simulate_keystrokes("cmd-2");
    settle(cx);
    let b = cx.debug_bounds("row-p1").expect("row");
    cx.simulate_click(b.center(), gpui::Modifiers::none());
    settle(cx);
    let state = app.read_with(cx, |a, cx| a.reports.read(cx).children.get("project:p1").cloned());
    assert!(matches!(state, Some(crate::reports::Children::Failed(_))), "{state:?}");
}
