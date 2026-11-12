use crate::core::interaction::DragState;
use crate::testing::{Fixture, insert_entry, open_app_db, wait_io};
use chrono::{Local, NaiveDate, TimeZone};
use gpui::{Modifiers, MouseButton, TestAppContext, point, px};

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
}

fn at(d: u32, h: u32, m: u32) -> chrono::DateTime<chrono::Utc> {
    Local.with_ymd_and_hms(2026, 11, d, h, m, 0).unwrap().to_utc()
}

/// Calendar on 2026-11-12 (Thu, 稼働日 → Mon 9 .. Fri 13) with entry "a" (10:00–11:00, Alpha).
/// Older entries give the picker title suggestions: Alpha 「定例会議」(newest) 「定例レビュー」「設計」,
/// no project 「ていれい会議」「設計」.
fn setup(cx: &mut TestAppContext) -> (gpui::Entity<crate::app::TrackApp>, &mut gpui::VisualTestContext, Fixture) {
    let (app, vcx, f) = open_app_db(
        cx,
        |db| {
            insert_entry(db, "a", at(12, 10, 0), at(12, 11, 0), Some("p1"), "作業a");
            insert_entry(db, "t3", local_day(2, 9), local_day(2, 10), Some("p1"), "設計");
            insert_entry(db, "t2", local_day(3, 9), local_day(3, 10), Some("p1"), "定例レビュー");
            insert_entry(db, "t1", local_day(4, 9), local_day(4, 10), Some("p1"), "定例会議");
            insert_entry(db, "n2", local_day(2, 11), local_day(2, 12), None, "設計");
            insert_entry(db, "n1", local_day(3, 11), local_day(3, 12), None, "ていれい会議");
        },
    );
    app.update(vcx, |a, cx| {
        a.calendar.update(cx, |c, cx| {
            c.anchor = day(12);
            cx.notify();
        })
    });
    wait_io(vcx);
    (app, vcx, f)
}

fn local_day(d: u32, h: u32) -> chrono::DateTime<chrono::Utc> {
    at(d, h, 0)
}

fn stored(f: &Fixture, id: &str) -> Option<track_core::model::TimeEntry> {
    f.db().entry(id).unwrap()
}

fn click(cx: &mut gpui::VisualTestContext, selector: &'static str) {
    let b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} not rendered"));
    cx.simulate_click(b.center(), Modifiers::none());
}

#[gpui::test]
fn drag_on_empty_space_opens_picker_placeholder(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
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
fn moving_entry_across_days_is_saved(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    drag_entry_one_column_right_and_30_minutes_down(cx);
    wait_io(cx);
    let e = stored(&f, "a").unwrap();
    assert_eq!((e.start, e.end), (at(13, 10, 30), at(13, 11, 30)));
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").map(|e| e.start)), Some(at(13, 10, 30)));
}

#[gpui::test]
fn moving_entry_rolls_back_when_saving_fails(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    f.data.fail_ops(&["update_entry"]);
    drag_entry_one_column_right_and_30_minutes_down(cx);
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").map(|e| e.start)), Some(at(12, 10, 0)));
    assert_eq!(stored(&f, "a").unwrap().start, at(12, 10, 0));
}

#[gpui::test]
fn arrow_keys_nudge_selected_entry(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_click(b.center(), Modifiers::none());
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).selected.clone()), Some("a".into()));
    cx.simulate_keystrokes("down");
    wait_io(cx);
    assert_eq!(stored(&f, "a").unwrap().start, at(12, 10, 15));
    cx.simulate_keystrokes("shift-down");
    wait_io(cx);
    assert_eq!(stored(&f, "a").unwrap().end, at(12, 11, 30));
    cx.simulate_keystrokes("escape");
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).selected.clone()), None);
}

#[gpui::test]
fn resize_bottom_edge_extends_entry(cx: &mut TestAppContext) {
    let (_app, cx, f) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    let edge = point(b.center().x, b.origin.y + b.size.height - px(2.));
    cx.simulate_mouse_down(edge, MouseButton::Left, Modifiers::none());
    let to = point(edge.x, edge.y + px(48.));
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    wait_io(cx);
    let e = stored(&f, "a").unwrap();
    assert_eq!((e.start, e.end), (at(12, 10, 0), at(12, 11, 30)));
}

#[gpui::test]
fn delete_key_removes_entry(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_click(b.center(), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    wait_io(cx);
    assert!(stored(&f, "a").is_none());
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").is_none()));
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).selected.clone()), None);
}

#[gpui::test]
fn delete_restores_entry_when_saving_fails(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    f.data.fail_ops(&["delete_entry"]);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_click(b.center(), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    wait_io(cx);
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("a").is_some()));
    assert!(stored(&f, "a").is_some());
}

fn drag_create_one_hour(cx: &mut gpui::VisualTestContext) {
    let b = cx.debug_bounds("entry-a").expect("entry block");
    let start = point(b.center().x, b.origin.y + px(96. * 3.)); // 10:00 entry → 13:xx area
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(start.x, start.y + px(96.)), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(point(start.x, start.y + px(96.)), MouseButton::Left, Modifiers::none());
}

/// Entries on the 12th other than the fixture's "a".
fn created_on_12th(f: &Fixture) -> Vec<track_core::model::TimeEntry> {
    f.db().entries(at(12, 0, 0), at(13, 0, 0)).unwrap().into_iter().filter(|e| e.id != "a").collect()
}

#[gpui::test]
fn picker_creates_entry_with_project_title_and_locked_tag(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    drag_create_one_hour(cx);
    assert!(cx.debug_bounds("project-picker").is_some(), "picker shown");
    click(cx, "project-p1");
    wait_io(cx); // titles
    cx.simulate_input("定例");
    cx.simulate_keystrokes("down enter"); // first suggestion → 定例会議
    cx.simulate_keystrokes("cmd-enter");
    wait_io(cx);
    let created = created_on_12th(&f);
    assert_eq!(created.len(), 1);
    let e = &created[0];
    assert_eq!(e.project_id.as_deref(), Some("p1"));
    assert_eq!(e.title.as_deref(), Some("定例会議"));
    assert_eq!(e.tags.iter().map(|t| t.tag_id.as_str()).collect::<Vec<_>>(), vec!["t1"]); // Alpha's project tag is locked on
    assert_eq!((e.end - e.start).num_minutes(), 60);
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).picker.is_none()));
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).drag.clone()), DragState::Idle);
}

#[gpui::test]
fn picker_ignores_enter_while_ime_is_composing(cx: &mut TestAppContext) {
    // The composing text 「てい」 prefixes a suggestion, so without the guard Enter would accept it.
    let (app, cx, f) = setup(cx);
    drag_create_one_hour(cx);
    click(cx, "project-none");
    wait_io(cx);
    cx.simulate_keystrokes("down"); // highlight first suggestion
    let picker = app.read_with(cx, |a, cx| a.calendar.read(cx).picker.as_ref().unwrap().0.clone());
    let title = picker.read_with(cx, |p, _| p.title_state());
    cx.update(|window, cx| {
        use gpui::EntityInputHandler;
        title.update(cx, |s, cx| s.replace_and_mark_text_in_range(None, "てい", None, window, cx));
    });
    cx.simulate_keystrokes("enter");
    cx.simulate_keystrokes("cmd-enter");
    wait_io(cx);
    let created = created_on_12th(&f);
    assert!(created.iter().all(|e| e.title.as_deref() != Some("ていれい会議")), "{created:?}");
}

#[gpui::test]
fn picker_escape_cancels(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    drag_create_one_hour(cx);
    cx.simulate_keystrokes("escape");
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).picker.is_none()));
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).drag.clone()), DragState::Idle);
}

#[gpui::test]
fn edit_dialog_saves_changes(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_click(b.center(), Modifiers::none());
    assert_eq!(cx.debug_bounds("entry-a"), Some(b), "selecting must not change the block's geometry");
    cx.simulate_mouse_move(b.center(), None, Modifiers::none());
    cx.simulate_event(gpui::MouseDownEvent { button: MouseButton::Left, position: b.center(), modifiers: Modifiers::none(), click_count: 2, first_mouse: false });
    cx.simulate_event(gpui::MouseUpEvent { button: MouseButton::Left, position: b.center(), modifiers: Modifiers::none(), click_count: 2 });
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).editing.clone()), Some("a".into()));
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("新タイトル");
    click(cx, "edit-project-button");
    click(cx, "project-p2");
    click(cx, "edit-save");
    wait_io(cx);
    let e = stored(&f, "a").unwrap();
    assert_eq!(e.title.as_deref(), Some("新タイトル"));
    assert_eq!(e.project_id.as_deref(), Some("p2"));
    assert_eq!((e.note.clone(), e.break_minutes, e.start), (None, 0, at(12, 10, 0)));
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).editing.clone()), None);
}

#[gpui::test]
fn changing_period_mid_drag_cancels_instead_of_saving_elsewhere(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_mouse_down(b.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(b.center().x, b.center().y + px(48.)), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_keystrokes("cmd-]");
    cx.simulate_mouse_up(point(b.center().x, b.center().y + px(48.)), MouseButton::Left, Modifiers::none());
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).drag.clone()), DragState::Idle);
    let e = stored(&f, "a").unwrap();
    assert_eq!((e.start, e.end), (at(12, 10, 0), at(12, 11, 0)), "no save after navigating away");
}
