use crate::core::interaction::DragState;
use crate::testing::{entry, fake_data, hanging_server, open_app_with};
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
