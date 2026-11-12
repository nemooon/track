use crate::core::interaction::DragState;
use crate::testing::{entry, fake_data, hanging_server, open_app_with, stub_server};
use chrono::{Local, NaiveDate, TimeZone};
use gpui::{Modifiers, MouseButton, TestAppContext, point, px};

fn wait_io(cx: &mut gpui::VisualTestContext) {
    for _ in 0..100 {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Opens the calendar on 2026-11-12 (Thu, 稼働日 view → Mon 9 .. Fri 13) with two entries.
fn setup(cx: &mut TestAppContext) -> (gpui::Entity<crate::app::TrackApp>, &mut gpui::VisualTestContext) {
    setup_with(cx, "http://127.0.0.1:9/api".into())
}

fn setup_with(cx: &mut TestAppContext, base: String) -> (gpui::Entity<crate::app::TrackApp>, &mut gpui::VisualTestContext) {
    cx.executor().allow_parking();
    let (app, vcx) = open_app_with(cx, base);
    let p = fake_data().projects[0].clone();
    app.update(vcx, |a, cx| {
        a.calendar.update(cx, |c, cx| {
            c.anchor = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
            c.cache = crate::core::interaction::EntryCache::new(vec![entry("a", 12, (10, 0), (11, 0), Some(&p))]);
            cx.notify();
        })
    });
    vcx.run_until_parked();
    // keep the fetched (failing) load from overwriting the cache
    app.update(vcx, |a, cx| a.calendar.update(cx, |c, _| c.block_reload_for_tests()));
    (app, vcx)
}

#[gpui::test]
fn drag_on_empty_space_opens_picker_placeholder(cx: &mut TestAppContext) {
    let (app, cx) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    // Drag 1 hour on the same column well below the entry.
    let x = b.center().x;
    let start = point(x, b.origin.y + px(96. * 3.));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(x, start.y + px(96.)), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(point(x, start.y + px(96.)), MouseButton::Left, Modifiers::none());
    let drag = app.read_with(cx, |a, cx| a.calendar.read(cx).drag.clone());
    let DragState::Picking { day_index, start_min, end_min } = drag else { panic!("expected Picking, got {drag:?}") };
    assert_eq!(day_index, 3);
    assert_eq!(end_min - start_min, 60);
    cx.simulate_keystrokes("escape");
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).drag.clone()), DragState::Idle);
}

fn drag_entry_one_column_right_and_30_minutes_down(cx: &mut gpui::VisualTestContext) {
    let b = cx.debug_bounds("entry-a").expect("entry block");
    let grab = b.center();
    cx.simulate_mouse_down(grab, MouseButton::Left, Modifiers::none());
    let col_w = b.size.width + px(6.);
    let to = point(grab.x + col_w, grab.y + px(48.));
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
}

#[gpui::test]
fn moving_entry_across_days_applies_immediately(cx: &mut TestAppContext) {
    let (app, cx) = setup_with(cx, hanging_server());
    drag_entry_one_column_right_and_30_minutes_down(cx);
    let e = app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").unwrap().clone());
    assert_eq!(e.start, Local.with_ymd_and_hms(2026, 11, 13, 10, 30, 0).unwrap().to_utc());
    assert_eq!(e.end, Local.with_ymd_and_hms(2026, 11, 13, 11, 30, 0).unwrap().to_utc());
}

#[gpui::test]
fn moving_entry_rolls_back_when_server_fails(cx: &mut TestAppContext) {
    let (app, cx) = setup(cx);
    drag_entry_one_column_right_and_30_minutes_down(cx);
    wait_io(cx);
    let back = app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").unwrap().start);
    assert_eq!(back, Local.with_ymd_and_hms(2026, 11, 12, 10, 0, 0).unwrap().to_utc());
}

#[gpui::test]
fn arrow_keys_nudge_selected_entry(cx: &mut TestAppContext) {
    let (app, cx) = setup_with(cx, hanging_server());
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_click(b.center(), Modifiers::none());
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).selected.clone()), Some("a".into()));
    cx.simulate_keystrokes("down");
    let e = app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").unwrap().clone());
    assert_eq!(e.start, Local.with_ymd_and_hms(2026, 11, 12, 10, 15, 0).unwrap().to_utc());
    cx.simulate_keystrokes("shift-down");
    let e = app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").unwrap().clone());
    assert_eq!(e.end, Local.with_ymd_and_hms(2026, 11, 12, 11, 30, 0).unwrap().to_utc());
    cx.simulate_keystrokes("escape");
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).selected.clone()), None);
}

#[gpui::test]
fn resize_bottom_edge_extends_entry(cx: &mut TestAppContext) {
    let (app, cx) = setup_with(cx, hanging_server());
    let b = cx.debug_bounds("entry-a").expect("entry block");
    let edge = point(b.center().x, b.origin.y + b.size.height - px(2.));
    cx.simulate_mouse_down(edge, MouseButton::Left, Modifiers::none());
    let to = point(edge.x, edge.y + px(48.));
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    let e = app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").unwrap().clone());
    assert_eq!(e.start, Local.with_ymd_and_hms(2026, 11, 12, 10, 0, 0).unwrap().to_utc());
    assert_eq!(e.end, Local.with_ymd_and_hms(2026, 11, 12, 11, 30, 0).unwrap().to_utc());
}

#[gpui::test]
fn delete_key_removes_entry_optimistically(cx: &mut TestAppContext) {
    let (app, cx) = setup_with(cx, hanging_server());
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_click(b.center(), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").is_none()));
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).selected.clone()), None);
}

#[gpui::test]
fn delete_restores_entry_when_server_fails(cx: &mut TestAppContext) {
    let (app, cx) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_click(b.center(), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    wait_io(cx);
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").is_some()));
}


fn click(cx: &mut gpui::VisualTestContext, selector: &'static str) {
    let b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} not rendered"));
    cx.simulate_click(b.center(), Modifiers::none());
}

fn titles_stub() -> (String, std::sync::Arc<std::sync::Mutex<Vec<crate::testing::Recorded>>>) {
    stub_server(|method, path| match (method, path) {
        ("GET", "/api/entries/titles") => (200, r#"["定例会議","定例レビュー","設計"]"#.into()),
        _ => (500, r#"{"error":"stub"}"#.into()),
    })
}

fn last_body(log: &std::sync::Arc<std::sync::Mutex<Vec<crate::testing::Recorded>>>, method: &str) -> serde_json::Value {
    let rec = log.lock().unwrap().iter().rev().find(|r| r.method == method).cloned().expect("request recorded");
    serde_json::from_str(&rec.body).unwrap()
}

fn drag_create_one_hour(cx: &mut gpui::VisualTestContext) {
    let b = cx.debug_bounds("entry-a").expect("entry block");
    let start = point(b.center().x, b.origin.y + px(96. * 3.)); // 10:00 entry → 13:xx area
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(start.x, start.y + px(96.)), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(point(start.x, start.y + px(96.)), MouseButton::Left, Modifiers::none());
}

#[gpui::test]
fn picker_creates_entry_with_project_title_and_locked_tag(cx: &mut TestAppContext) {
    let (base, log) = titles_stub();
    let (app, cx) = setup_with(cx, base);
    drag_create_one_hour(cx);
    assert!(cx.debug_bounds("project-picker").is_some(), "picker shown");
    click(cx, "project-p1");
    wait_io(cx); // titles
    cx.simulate_input("定例");
    cx.simulate_keystrokes("down enter"); // first suggestion → 定例会議
    cx.simulate_keystrokes("cmd-enter");
    wait_io(cx);
    let body = last_body(&log, "POST");
    assert_eq!(body["projectId"], "p1");
    assert_eq!(body["title"], "定例会議");
    assert_eq!(body["tagIds"], serde_json::json!(["t1"])); // Alpha's project tag is locked on
    let (s, e) = (body["start"].as_str().unwrap(), body["end"].as_str().unwrap());
    let dur = chrono::DateTime::parse_from_rfc3339(e).unwrap() - chrono::DateTime::parse_from_rfc3339(s).unwrap();
    assert_eq!(dur.num_minutes(), 60);
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).picker.is_none()));
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).drag.clone()), DragState::Idle);
}

#[gpui::test]
fn picker_ignores_enter_while_ime_is_composing(cx: &mut TestAppContext) {
    // The composing text 「てい」 prefixes a suggestion, so without the guard Enter would accept it.
    let (base, log) = stub_server(|method, path| match (method, path) {
        ("GET", "/api/entries/titles") => (200, r#"["ていれい会議","設計"]"#.into()),
        _ => (500, r#"{"error":"stub"}"#.into()),
    });
    let (app, cx) = setup_with(cx, base);
    drag_create_one_hour(cx);
    click(cx, "project-none");
    wait_io(cx);
    cx.simulate_keystrokes("down"); // highlight first suggestion
    // Start an IME composition (marked text) and press Enter: it must not pick the suggestion.
    let picker = app.read_with(cx, |a, cx| a.calendar.read(cx).picker.as_ref().unwrap().0.clone());
    let title = picker.read_with(cx, |p, _| p.title_state());
    cx.update(|window, cx| {
        use gpui::EntityInputHandler;
        title.update(cx, |s, cx| s.replace_and_mark_text_in_range(None, "てい", None, window, cx));
    });
    cx.simulate_keystrokes("enter");
    cx.simulate_keystrokes("cmd-enter");
    wait_io(cx);
    let body = last_body(&log, "POST");
    assert_ne!(body.get("title").and_then(|t| t.as_str()), Some("ていれい会議"));
}

#[gpui::test]
fn picker_escape_cancels(cx: &mut TestAppContext) {
    let (base, _log) = titles_stub();
    let (app, cx) = setup_with(cx, base);
    drag_create_one_hour(cx);
    cx.simulate_keystrokes("escape");
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).picker.is_none()));
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).drag.clone()), DragState::Idle);
}

#[gpui::test]
fn edit_dialog_saves_patch(cx: &mut TestAppContext) {
    let (base, log) = titles_stub();
    let (app, cx) = setup_with(cx, base);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_click(b.center(), Modifiers::none());
    assert_eq!(cx.debug_bounds("entry-a"), Some(b), "selecting must not change the block's geometry");
    cx.simulate_mouse_move(b.center(), None, Modifiers::none());
    cx.simulate_event(gpui::MouseDownEvent { button: MouseButton::Left, position: b.center(), modifiers: Modifiers::none(), click_count: 2, first_mouse: false });
    cx.simulate_event(gpui::MouseUpEvent { button: MouseButton::Left, position: b.center(), modifiers: Modifiers::none(), click_count: 2 });
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).editing.clone()), Some("a".into()));
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("新タイトル");
    // switch project to Beta: its locked tags are none, existing tags kept
    click(cx, "edit-project-button");
    click(cx, "project-p2");
    click(cx, "edit-save");
    wait_io(cx);
    let body = last_body(&log, "PATCH");
    assert_eq!(body["title"], "新タイトル");
    assert_eq!(body["projectId"], "p2");
    assert_eq!(body["note"], serde_json::Value::Null);
    assert_eq!(body["breakMinutes"], 0);
    assert_eq!(chrono::DateTime::parse_from_rfc3339(body["start"].as_str().unwrap()).unwrap(), Local.with_ymd_and_hms(2026, 11, 12, 10, 0, 0).unwrap());
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).editing.clone()), None);
}
