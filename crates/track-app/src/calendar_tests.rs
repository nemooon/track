use crate::core::interaction::DragState;
use crate::testing::{Fixture, insert_entry, open_app_db, wait_io};
use chrono::{Local, NaiveDate, TimeZone};
use gpui::{Modifiers, MouseButton, TestAppContext, point, px};

/// Pixels per hour at the default zoom.
const HOUR: f32 = crate::core::geometry::ZOOM_LEVELS[crate::core::geometry::DEFAULT_ZOOM];

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
    let start = point(x, b.origin.y + px(HOUR * 3.));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(x, start.y + px(HOUR)), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(point(x, start.y + px(HOUR)), MouseButton::Left, Modifiers::none());
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
    let to = point(grab.x + col_w, grab.y + px(HOUR / 2.));
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
    let to = point(edge.x, edge.y + px(HOUR / 2.));
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
    let start = point(b.center().x, b.origin.y + px(HOUR * 3.)); // 10:00 entry → 13:xx area
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(start.x, start.y + px(HOUR)), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(point(start.x, start.y + px(HOUR)), MouseButton::Left, Modifiers::none());
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
    cx.simulate_mouse_move(point(b.center().x, b.center().y + px(HOUR / 2.)), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_keystrokes("cmd-]");
    cx.simulate_mouse_up(point(b.center().x, b.center().y + px(HOUR / 2.)), MouseButton::Left, Modifiers::none());
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).drag.clone()), DragState::Idle);
    let e = stored(&f, "a").unwrap();
    assert_eq!((e.start, e.end), (at(12, 10, 0), at(12, 11, 0)), "no save after navigating away");
}

/// The hover time pill sits left of the line, on one line.
#[gpui::test]
fn hover_time_label_is_one_line_left_of_the_line(cx: &mut TestAppContext) {
    let (_app, cx, _f) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    let at = point(b.center().x, b.origin.y + px(HOUR * 3.));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    let label = cx.debug_bounds("hover-time").expect("hover label");
    assert!(label.size.height <= px(16.), "single line, got {:?}", label.size);
    assert!(label.size.width >= px(24.), "fits HH:MM, got {:?}", label.size);
}

fn visible_days(app: &gpui::Entity<crate::app::TrackApp>, cx: &mut gpui::VisualTestContext) -> Vec<NaiveDate> {
    cx.update(|w, cx| app.read(cx).calendar.read(cx).days(w, cx))
}

#[gpui::test]
fn visible_total_counts_only_shown_days(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    let days = visible_days(&app, cx);
    let total = |app: &gpui::Entity<crate::app::TrackApp>, cx: &mut gpui::VisualTestContext, days: &[NaiveDate]| app.read_with(cx, |a, cx| a.calendar.read(cx).visible_total_minutes(days));
    assert_eq!(total(&app, cx, &days), 60, "only entry a (12th) is in the shown week");
    insert_entry(&f.db(), "late", at(12, 23, 0), at(13, 0, 0), None, "深夜");
    insert_entry(&f.db(), "before", at(8, 23, 0), at(9, 0, 0), None, "前週");
    app.update(cx, |a, cx| a.calendar.update(cx, |c, cx| c.reload(cx)));
    wait_io(cx);
    assert_eq!(total(&app, cx, &days), 120, "an entry ending at midnight counts on its day; the day before the range does not");
}

#[gpui::test]
fn selection_label_shows_title_and_span(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).selection_label()), None);
    click(cx, "entry-a");
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).selection_label()), Some("作業a 10:00–11:00（1:00）".into()));
    cx.simulate_keystrokes("escape");
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).selection_label()), None);
}

#[gpui::test]
fn status_bar_shows_calendar_total(cx: &mut TestAppContext) {
    let (_app, cx, _f) = setup(cx);
    let bar = cx.debug_bounds("status-bar").expect("status bar");
    let total = cx.debug_bounds("status-total").expect("calendar total in the status bar");
    assert!(bar.contains(&total.center()));
}

#[gpui::test]
fn selection_leaves_the_status_bar_when_off_screen(cx: &mut TestAppContext) {
    let (_app, cx, _f) = setup(cx);
    click(cx, "entry-a");
    assert!(cx.debug_bounds("status-selection").is_some());
    cx.simulate_keystrokes("cmd-]");
    wait_io(cx);
    assert!(cx.debug_bounds("status-selection").is_none(), "the selected entry is not on screen any more");
}

/// At the default (middle) zoom a 15-minute entry shows one full line: border 2 + padding 8 + line 15.
#[gpui::test]
fn fifteen_minute_entry_fits_one_line_at_default_zoom(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    insert_entry(&f.db(), "q", at(12, 14, 0), at(12, 14, 15), None, "短い作業");
    app.update(cx, |a, cx| a.calendar.update(cx, |c, cx| c.reload(cx)));
    wait_io(cx);
    let h = cx.debug_bounds("entry-q").expect("15-minute entry").size.height;
    assert!(h >= px(25.), "one line needs 25px, got {h:?}");
}

/// Title with the duration at its right end, then project below, left-aligned; no time range.
#[gpui::test]
fn entry_block_puts_duration_beside_title_and_project_below(cx: &mut TestAppContext) {
    let (_app, cx, _f) = setup(cx);
    let get = |cx: &mut gpui::VisualTestContext, s: &'static str| cx.debug_bounds(s).unwrap_or_else(|| panic!("{s}"));
    let (title, dur, project) = (get(cx, "entry-a-title"), get(cx, "entry-a-duration"), get(cx, "entry-a-project"));
    let block = get(cx, "entry-a");
    assert!((dur.center().y - title.center().y).abs() <= px(2.), "duration on the title's line: {title:?} {dur:?}");
    assert!(dur.origin.x >= title.origin.x + title.size.width - px(1.), "duration right of the title");
    assert!(block.origin.x + block.size.width - (dur.origin.x + dur.size.width) <= px(12.), "duration at the right end");
    assert!(project.origin.y >= title.origin.y + title.size.height - px(1.), "project below the title");
    assert!((project.origin.x - title.origin.x).abs() <= px(1.), "project left-aligned with the title");
    assert!(cx.debug_bounds("entry-a-span").is_none(), "no start–end time in the block");
}

#[gpui::test]
fn calendar_durations_follow_the_report_style(cx: &mut TestAppContext) {
    use crate::persist::DurationStyle;
    let (app, cx, _f) = setup(cx);
    for (style, want) in [(DurationStyle::Decimal, "1.5h"), (DurationStyle::HoursMinutes, "1h 30m")] {
        app.update(cx, |a, cx| a.state.update(cx, |s, _| s.update_persist(|p| p.duration_style = style)));
        assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).duration(90, cx)), want);
    }
}

#[gpui::test]
fn the_day_header_is_one_row_and_zoom_sits_in_the_toolbar(cx: &mut TestAppContext) {
    let (_app, cx, _f) = setup(cx);
    let header = cx.debug_bounds("day-header").expect("day header");
    assert!(header.size.height <= px(40.), "{:?}", header.size.height);
    let zoom = cx.debug_bounds("zoom-in").expect("zoom button");
    assert!(zoom.bottom() <= header.top(), "zoom is in the toolbar above the header");
}

#[gpui::test]
fn a_narrow_short_block_still_shows_its_title(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    // Three 15-minute entries side by side on Thursday: each is narrow.
    for id in ["n-a", "n-b", "n-c"] {
        insert_entry(&f.db(), id, at(12, 10, 0), at(12, 10, 15), Some("p1"), "狭いエントリー");
    }
    app.update(cx, |a, cx| a.calendar.update(cx, |c, cx| c.reload(cx)));
    wait_io(cx);
    let title = cx.debug_bounds("entry-n-b-title").expect("title rendered");
    assert!(title.size.height >= px(10.), "{:?}", title.size);
}

fn click_id(cx: &mut gpui::VisualTestContext, id: String) {
    let sel: &'static str = Box::leak(id.into_boxed_str());
    let b = cx.debug_bounds(sel).unwrap_or_else(|| panic!("{sel} not rendered"));
    cx.simulate_click(b.center(), Modifiers::none());
}

#[gpui::test]
fn choosing_full_leave_marks_the_day(cx: &mut TestAppContext) {
    let (_app, cx, f) = setup(cx);
    click_id(cx, "day-menu-2026-11-12".into());
    click_id(cx, "day-leave-full".into());
    wait_io(cx);
    let leaves = f.db().leaves(day(12), day(13)).unwrap();
    assert_eq!(leaves.first().map(|l| l.kind), Some(track_core::model::LeaveKind::Full));
    assert!(cx.debug_bounds("day-mark-2026-11-12").is_some(), "the header says 休暇");
}

#[gpui::test]
fn direct_shows_in_the_time_grid_and_counts_once(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    let days = visible_days(&app, cx);
    let before = app.read_with(cx, |a, cx| a.calendar.read(cx).visible_total_minutes(&days));
    click_id(cx, "day-menu-2026-11-12".into());
    click_id(cx, "project-p1".into());
    wait_io(cx);
    let direct = f.db().direct_entry(day(12)).unwrap().expect("direct entry");
    assert!(cx.debug_bounds(Box::leak(format!("entry-{}", direct.id).into_boxed_str())).is_some(), "a block in the time grid");
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).visible_total_minutes(&days)), before + 450);
    assert!(cx.debug_bounds("day-mark-2026-11-12").is_some(), "the header says 直行直帰");
}

#[gpui::test]
fn direct_uses_the_shared_project_list(cx: &mut TestAppContext) {
    let (_app, cx, f) = setup(cx);
    click_id(cx, "day-menu-2026-11-12".into());
    assert!(cx.debug_bounds("project-p1").is_some() && cx.debug_bounds("project-p2").is_some());
    cx.simulate_input("bet");
    assert!(cx.debug_bounds("project-p1").is_none(), "the search narrows the list");
    click_id(cx, "project-p2".into());
    wait_io(cx);
    assert_eq!(f.db().direct_entry(day(12)).unwrap().and_then(|e| e.project_id).as_deref(), Some("p2"));
}

#[gpui::test]
fn direct_can_be_made_without_a_project(cx: &mut TestAppContext) {
    let (_app, cx, f) = setup(cx);
    click_id(cx, "day-menu-2026-11-12".into());
    click_id(cx, "project-none".into());
    wait_io(cx);
    let e = f.db().direct_entry(day(12)).unwrap().expect("direct entry");
    assert_eq!(e.project_id, None);
    assert!(cx.debug_bounds("day-mark-2026-11-12").is_some(), "the header says 直行直帰");
}

#[gpui::test]
fn holiday_none_option_cancels_the_holiday_leave(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    app.update(cx, |a, cx| a.calendar.update(cx, |c, cx| {
        c.anchor = day(3);
        cx.notify();
    }));
    wait_io(cx);
    click_id(cx, "day-menu-2026-11-03".into());
    click_id(cx, "day-leave-none".into());
    wait_io(cx);
    assert_eq!(f.db().leave_minutes(day(3), day(4)).unwrap()[0].1, 0);
}

#[test]
fn work_band_leaves_out_leave() {
    use crate::calendar::work_band;
    use track_core::model::LeaveKind;
    let s = track_core::model::Settings { work_start: 600, work_end: 1110, work_days: vec![1, 2, 3, 4, 5], weekly_report_template: String::new(), report_copy_formats: vec![] };
    let d = |m: u32, day: u32| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
    assert_eq!(work_band(&s, d(11, 12), None), Some((600, 1110)), "a plain work day");
    assert_eq!(work_band(&s, d(11, 15), None), None, "Sunday");
    assert_eq!(work_band(&s, d(11, 12), Some(LeaveKind::Full)), None, "full leave");
    assert_eq!(work_band(&s, d(11, 12), Some(LeaveKind::Am)), Some((855, 1110)), "前半休: the second half");
    assert_eq!(work_band(&s, d(11, 12), Some(LeaveKind::Pm)), Some((600, 855)), "後半休: the first half");
    assert_eq!(work_band(&s, d(11, 3), None), None, "a holiday is leave");
    assert_eq!(work_band(&s, d(11, 3), Some(LeaveKind::None)), Some((600, 1110)), "「休暇にしない」 on a holiday");
}

/// Off hours darken toward midnight only outside the work hours (0:00 → work start upward, work
/// end → 24:00 downward). Inside the work hours, a part that is not worked (a day off, a half day)
/// is one flat shade, the lightest.
#[test]
fn off_hours_darken_toward_midnight() {
    use crate::calendar::{Shade, off_hours_spans};
    let hours = (600, 1110);
    assert_eq!(off_hours_spans(hours, Some((600, 1110))), vec![(0, 600, Shade::DarkTop), (1110, 1440, Shade::DarkBottom)]);
    assert_eq!(
        off_hours_spans(hours, None),
        vec![(0, 600, Shade::DarkTop), (600, 1110, Shade::Flat), (1110, 1440, Shade::DarkBottom)],
        "a day off: flat through the work hours"
    );
    assert_eq!(
        off_hours_spans(hours, Some((855, 1110))),
        vec![(0, 600, Shade::DarkTop), (600, 855, Shade::Flat), (1110, 1440, Shade::DarkBottom)],
        "前半休: the morning is flat"
    );
    assert_eq!(
        off_hours_spans(hours, Some((600, 855))),
        vec![(0, 600, Shade::DarkTop), (855, 1110, Shade::Flat), (1110, 1440, Shade::DarkBottom)],
        "後半休: the afternoon is flat"
    );
}

/// The grid lines darken with the background: none in worked hours, the lightest step next to the
/// work hours and through a day off, the darkest at midnight, in proportion between.
#[test]
fn off_hours_alpha_follows_the_gradient() {
    use crate::calendar::{OFF_HOURS_DARKEST as D, OFF_HOURS_LIGHTEST as L, off_hours_alpha};
    let (hours, band) = ((600, 1110), Some((600, 1110)));
    assert_eq!(off_hours_alpha(800, hours, band), 0.);
    assert_eq!(off_hours_alpha(0, hours, band), D);
    assert_eq!(off_hours_alpha(1440, hours, band), D);
    assert_eq!(off_hours_alpha(600, hours, band), L);
    assert_eq!(off_hours_alpha(1110, hours, band), L);
    assert!((off_hours_alpha(300, hours, band) - (L + D) / 2.).abs() < 1e-6);
    assert_eq!(off_hours_alpha(800, hours, None), L, "a day off is flat in the work hours");
    assert_eq!(off_hours_alpha(300, hours, None), off_hours_alpha(300, hours, band), "same gradient as a work day");
    use crate::calendar::grid::{LINE_TINT, off_hours_line_alpha};
    assert!(off_hours_line_alpha(L) > LINE_TINT && off_hours_line_alpha(D) > off_hours_line_alpha(L), "lines deepen with the background");
}

/// In both themes the work hours are the lightest part of the calendar: off hours get darker
/// toward midnight, and their grid lines darker with them while staying apart from the background.
#[test]
fn off_hours_get_darker_in_both_themes() {
    use crate::calendar::grid::{off_hours_bg, off_hours_line};
    use crate::calendar::{OFF_HOURS_DARKEST as D, OFF_HOURS_LIGHTEST as L};
    use crate::ui::theme::Palette;
    let lum = |c: gpui::Hsla| { let r = gpui::Rgba::from(c); 0.2126 * r.r + 0.7152 * r.g + 0.0722 * r.b };
    for (name, p) in [("light", Palette::light()), ("dark", Palette::dark())] {
        let (bg_l, bg_d) = (off_hours_bg(&p, L), off_hours_bg(&p, D));
        assert!(lum(bg_l) < lum(p.work_hours) && lum(bg_d) < lum(bg_l), "{name}: background darkens toward midnight");
        let (line_l, line_d) = (off_hours_line(&p, L), off_hours_line(&p, D));
        assert!(lum(line_d) < lum(line_l), "{name}: lines darken toward midnight");
        assert!((lum(line_d) - lum(bg_d)).abs() > 0.02, "{name}: lines stay visible at midnight");
    }
}

/// A 直行直帰 on the 13th (made through core, like the day menu does), with its block's selector.
fn direct_block(f: &Fixture, cx: &mut gpui::VisualTestContext) -> (track_core::model::TimeEntry, &'static str) {
    let e = f.db().set_direct(day(13), Some("p1")).unwrap();
    cx.update(|_, cx| cx.refresh_windows());
    wait_io(cx);
    (e.clone(), Box::leak(format!("entry-{}", e.id).into_boxed_str()))
}

#[gpui::test]
fn direct_blocks_never_move(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    app.update(cx, |a, cx| a.calendar.update(cx, |c, cx| c.reload(cx)));
    let (e, sel) = direct_block(&f, cx);
    app.update(cx, |a, cx| a.calendar.update(cx, |c, cx| c.reload(cx)));
    wait_io(cx);
    let b = cx.debug_bounds(sel).expect("direct block");
    // Drag the middle.
    cx.simulate_mouse_down(b.center(), MouseButton::Left, Modifiers::none());
    let to = point(b.center().x, b.center().y + px(HOUR));
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    wait_io(cx);
    // Drag the bottom edge.
    let edge = point(b.center().x, b.origin.y + b.size.height - px(2.));
    cx.simulate_mouse_down(edge, MouseButton::Left, Modifiers::none());
    let to = point(edge.x, edge.y + px(HOUR));
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    wait_io(cx);
    // The arrow keys.
    cx.simulate_click(b.center(), Modifiers::none());
    cx.simulate_keystrokes("down");
    cx.simulate_keystrokes("shift-down");
    wait_io(cx);
    let now = stored(&f, &e.id).unwrap();
    assert_eq!((now.start, now.end), (e.start, e.end));
}

#[gpui::test]
fn direct_edit_dialog_has_no_times(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    let (e, sel) = direct_block(&f, cx);
    app.update(cx, |a, cx| a.calendar.update(cx, |c, cx| c.reload(cx)));
    wait_io(cx);
    let b = cx.debug_bounds(sel).expect("direct block");
    cx.simulate_mouse_move(b.center(), None, Modifiers::none());
    cx.simulate_event(gpui::MouseDownEvent { button: MouseButton::Left, position: b.center(), modifiers: Modifiers::none(), click_count: 2, first_mouse: false });
    cx.simulate_event(gpui::MouseUpEvent { button: MouseButton::Left, position: b.center(), modifiers: Modifiers::none(), click_count: 2 });
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).editing.clone()), Some(e.id.clone()));
    assert!(cx.debug_bounds("edit-times").is_none(), "no start, end or break for a direct visit");
}

#[gpui::test]
fn normal_edit_dialog_has_times(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_mouse_move(b.center(), None, Modifiers::none());
    cx.simulate_event(gpui::MouseDownEvent { button: MouseButton::Left, position: b.center(), modifiers: Modifiers::none(), click_count: 2, first_mouse: false });
    cx.simulate_event(gpui::MouseUpEvent { button: MouseButton::Left, position: b.center(), modifiers: Modifiers::none(), click_count: 2 });
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).editing.clone()), Some("a".into()));
    assert!(cx.debug_bounds("edit-times").is_some());
}

#[gpui::test]
fn today_is_marked_by_a_line_on_top_of_its_header(cx: &mut TestAppContext) {
    use chrono::Datelike;
    // The current week (setup() moves to another one).
    let (_app, cx) = crate::testing::open_app(cx);
    let today = chrono::Local::now().date_naive();
    let bar = cx.debug_bounds("today-bar");
    if today.weekday().num_days_from_monday() >= 5 {
        assert!(bar.is_none(), "the default view hides weekends");
        return;
    }
    let bar = bar.expect("today's line");
    let header = cx.debug_bounds("day-header").unwrap();
    assert_eq!(bar.top(), header.top(), "on the top edge");
    assert!(bar.size.height >= gpui::px(2.) && bar.size.height <= gpui::px(3.));
    assert!(bar.size.width > gpui::px(50.), "spans the day's column");
}

#[gpui::test]
fn saving_an_entry_reshuffles_the_header_dots(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let seed = |cx: &mut gpui::VisualTestContext| app.read_with(cx, |a, cx| a.state.read(cx).header_seed);
    let shuffles = |cx: &mut gpui::VisualTestContext| app.read_with(cx, |a, cx| a.state.read(cx).header_shuffles);
    let before = seed(cx);
    let body = track_core::model::NewEntry { start: at(13, 9, 0), end: at(13, 10, 0), title: Some("新規".into()), ..Default::default() };
    cx.update(|window, cx| app.update(cx, |a, cx| a.calendar.update(cx, |c, cx| c.create(body, window, cx))));
    wait_io(cx);
    sparkle_out(cx);
    let created = seed(cx);
    assert_ne!(before, created, "creating changes the pattern");
    assert_eq!(shuffles(cx), crate::app::SPARKLE_STEPS, "it flickers through several patterns");
    cx.update(|window, cx| app.update(cx, |a, cx| a.calendar.update(cx, |c, cx| c.commit_times("a", at(12, 11, 0), at(12, 12, 0), window, cx))));
    wait_io(cx);
    sparkle_out(cx);
    assert_ne!(created, seed(cx), "updating changes it too");
}

#[gpui::test]
fn deleting_or_changing_leave_sparkles_too(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let shuffles = |cx: &mut gpui::VisualTestContext| app.read_with(cx, |a, cx| a.state.read(cx).header_shuffles);
    cx.update(|window, cx| app.update(cx, |a, cx| a.calendar.update(cx, |c, cx| c.delete("a", window, cx))));
    wait_io(cx);
    sparkle_out(cx);
    assert_eq!(shuffles(cx), crate::app::SPARKLE_STEPS, "deleting");
    let api = app.read_with(cx, |a, cx| a.state.read(cx).api.clone());
    gpui::block_on(api.set_leave(day(13), Some(track_core::model::LeaveKind::Full))).unwrap();
    wait_io(cx);
    sparkle_out(cx);
    assert_eq!(shuffles(cx), 2 * crate::app::SPARKLE_STEPS, "leave");
}

/// Runs the header's sparkle to the end (its timer runs on the test clock).
fn sparkle_out(cx: &mut gpui::VisualTestContext) {
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
}

fn scroll_y(app: &gpui::Entity<crate::app::TrackApp>, cx: &mut gpui::VisualTestContext) -> f32 {
    app.read_with(cx, |a, cx| a.calendar.read(cx).scroll_offset_y())
}

fn moving_start(app: &gpui::Entity<crate::app::TrackApp>, cx: &mut gpui::VisualTestContext) -> i64 {
    match app.read_with(cx, |a, cx| a.calendar.read(cx).drag.clone()) {
        DragState::Moving { start_min, .. } => start_min,
        other => panic!("expected Moving, got {other:?}"),
    }
}

/// Holding a dragged entry near the bottom of the grid keeps scrolling and carries the entry along,
/// even with the mouse still; letting go saves and stops the scrolling.
#[gpui::test]
fn dragging_to_the_bottom_edge_scrolls_and_carries_the_entry(cx: &mut TestAppContext) {
    let (app, cx, f) = setup(cx);
    let view = cx.debug_bounds("calendar-scroll").expect("scroll area");
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_mouse_down(b.center(), MouseButton::Left, Modifiers::none());
    let edge = point(b.center().x, view.bottom() - px(5.));
    cx.simulate_mouse_move(edge, Some(MouseButton::Left), Modifiers::none());
    let (y0, s0) = (scroll_y(&app, cx), moving_start(&app, cx));
    cx.executor().advance_clock(std::time::Duration::from_millis(500));
    let (y1, s1) = (scroll_y(&app, cx), moving_start(&app, cx));
    assert!(y1 < y0 - 100., "scrolled down: {y0} -> {y1}");
    assert!(s1 > s0, "entry follows the scroll: {s0} -> {s1}");
    cx.simulate_mouse_up(edge, MouseButton::Left, Modifiers::none());
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.calendar.read(cx).drag.clone()), DragState::Idle);
    assert_ne!(stored(&f, "a").unwrap().start, at(12, 10, 0), "saved where it was dropped");
    let y2 = scroll_y(&app, cx);
    cx.executor().advance_clock(std::time::Duration::from_millis(500));
    assert_eq!(scroll_y(&app, cx), y2, "no scrolling after letting go");
}

#[gpui::test]
fn dragging_to_the_top_edge_scrolls_up(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let view = cx.debug_bounds("calendar-scroll").expect("scroll area");
    let b = cx.debug_bounds("entry-a").expect("entry block");
    let y0 = scroll_y(&app, cx);
    assert!(y0 < -100., "starts scrolled into the day: {y0}");
    cx.simulate_mouse_down(b.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(b.center().x, view.top() + px(5.)), Some(MouseButton::Left), Modifiers::none());
    cx.executor().advance_clock(std::time::Duration::from_secs(5));
    assert_eq!(scroll_y(&app, cx), 0., "stops at the top");
    cx.simulate_mouse_up(point(b.center().x, view.top() + px(5.)), MouseButton::Left, Modifiers::none());
}

/// Each day's total sits in a footer under the grid, in line with that day's column.
#[gpui::test]
fn day_totals_sit_in_a_footer_under_their_columns(cx: &mut TestAppContext) {
    let (_app, cx, _f) = setup(cx);
    let grid = cx.debug_bounds("calendar-scroll").expect("scroll area");
    let footer = cx.debug_bounds("day-footer").expect("day footer");
    assert!(footer.top() >= grid.bottom() - px(1.), "footer under the grid: {footer:?} / {grid:?}");
    let total = cx.debug_bounds("day-total-2026-11-12").expect("total of the 12th");
    let entry = cx.debug_bounds("entry-a").expect("entry block");
    assert!(footer.top() <= total.top() && total.bottom() <= footer.bottom(), "in the footer");
    assert!(total.left() < entry.right() && entry.left() < total.right(), "under the 12th's column");
    assert!(cx.debug_bounds("day-total-2026-11-11").is_none(), "no total on an empty day");
}

/// A move that saves says nothing: the calendar already shows it and the header sparkles.
#[gpui::test]
fn a_successful_save_leaves_the_status_bar_quiet(cx: &mut TestAppContext) {
    let (_app, cx, f) = setup(cx);
    drag_entry_one_column_right_and_30_minutes_down(cx);
    wait_io(cx);
    assert_eq!(stored(&f, "a").unwrap().start, at(13, 10, 30));
    assert!(cx.debug_bounds("status-note").is_none(), "no notice for a plain save");
}

/// Deleting tells so in the status bar, with 「元に戻す」 bringing the entry back.
#[gpui::test]
fn deleting_offers_undo_in_the_status_bar(cx: &mut TestAppContext) {
    let (_app, cx, f) = setup(cx);
    let b = cx.debug_bounds("entry-a").expect("entry block");
    cx.simulate_click(b.center(), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    wait_io(cx);
    assert!(stored(&f, "a").is_none());
    assert!(cx.debug_bounds("status-note").is_some(), "the deletion is told in the status bar");
    click(cx, "status-note-undo");
    wait_io(cx);
    let back: Vec<_> = f.db().entries(at(12, 0, 0), at(13, 0, 0)).unwrap().into_iter().filter(|e| e.title.as_deref() == Some("作業a")).collect();
    assert_eq!(back.len(), 1, "the entry is made again");
    assert_eq!((back[0].start, back[0].end), (at(12, 10, 0), at(12, 11, 0)));
    assert!(cx.debug_bounds("status-note").is_none(), "the notice goes once undone");
}

/// A failed save is told in red in the status bar and stays until closed.
#[gpui::test]
fn a_failed_save_is_told_in_the_status_bar(cx: &mut TestAppContext) {
    let (_app, cx, f) = setup(cx);
    f.data.fail_ops(&["update_entry"]);
    drag_entry_one_column_right_and_30_minutes_down(cx);
    wait_io(cx);
    assert!(cx.debug_bounds("status-note-error").is_some(), "the failure is shown");
    click(cx, "status-note-close");
    assert!(cx.debug_bounds("status-note").is_none(), "closed");
}
