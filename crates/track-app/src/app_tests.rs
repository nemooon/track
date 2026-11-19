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
    let data = crate::data::DataClient::open_guarded(dir.path());
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
    let data = crate::data::DataClient::open_unguarded(dir.path());
    let (app, cx) = crate::testing::open_app_loading(cx, data);
    settle(cx);
    let names = app.read_with(cx, |a, cx| a.state.read(cx).data.as_ref().map(|d| d.projects.iter().map(|p| p.name.clone()).collect::<Vec<_>>()));
    assert_eq!(names, Some(vec!["Alpha".to_string(), "Beta".to_string()]));
}

#[gpui::test]
fn calendar_shows_error_when_entries_fail(cx: &mut TestAppContext) {
    let (app, cx, f) = crate::testing::open_app_db(cx, |_| {});
    f.data.fail_ops(&["entries"]);
    cx.simulate_keystrokes("cmd-]");
    settle(cx);
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).load_error.is_some()));
    assert!(cx.debug_bounds("calendar-retry").is_some(), "retry button shown");
}

#[gpui::test]
fn reports_show_error_instead_of_loading_forever(cx: &mut TestAppContext) {
    let (app, cx, f) = crate::testing::open_app_db(cx, |_| {});
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

#[gpui::test]
fn external_write_triggers_reload(cx: &mut TestAppContext) {
    use crate::testing::{insert_entry, local, open_app_db, wait_io};
    let (app, cx, fixture) = open_app_db(cx, |_| {});
    wait_io(cx);
    let today = crate::testing::visible_weekday();
    assert!(app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("ext").is_none()));
    // Another process (track-cli, AI) writes through its own connection.
    let other = fixture.db();
    insert_entry(&other, "ext", local(today, 10, 0), local(today, 11, 0), Some("p1"), "外部で登録");
    other
        .conn()
        .execute(r##"INSERT INTO "Project"(id, clientId, name, color, createdAt) VALUES ('p3', 'c1', 'Gamma', '#000000', '2026-11-12T00:00:00.000+00:00')"##, [])
        .unwrap();
    let mut seen = false;
    for _ in 0..30 {
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(10));
        if app.read_with(cx, |a, cx| a.calendar.read(cx).cache.get("ext").is_some()) {
            seen = true;
            break;
        }
    }
    assert!(seen, "external entry appears within 3 seconds");
    wait_io(cx);
    let projects = app.read_with(cx, |a, cx| a.state.read(cx).data.as_ref().map(|d| d.projects.iter().any(|p| p.id == "p3")));
    assert_eq!(projects, Some(true), "reference data is reloaded too");
}

#[gpui::test]
fn integrations_dialog_installs_dev_names(cx: &mut TestAppContext) {
    let (app, cx) = open_app(cx);
    let home = tempfile::tempdir().unwrap();
    app.update(cx, |a, _| a.integrations_home = home.path().to_path_buf());
    cx.dispatch_action(crate::actions::InstallIntegrations);
    cx.run_until_parked();
    let b = cx.debug_bounds("install-both").expect("dialog with choices");
    cx.simulate_click(b.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    assert!(home.path().join(".codex/skills/track-dev/SKILL.md").is_file());
    assert!(home.path().join(".claude/commands/track-dev.md").is_file());
    assert!(!home.path().join(".codex/skills/track").exists(), "the real skill name is never used");
    let status = app.read_with(cx, |a, cx| a.integrations.as_ref().map(|d| d.read(cx).status.clone()));
    assert!(matches!(status, Some(Some(Ok(ref paths))) if paths.len() == 2), "{status:?}");
}

#[test]
fn db_job_panic_does_not_close_data_access() {
    let dir = tempfile::tempdir().unwrap();
    let data = crate::data::DataClient::open_unguarded(dir.path());
    let err = futures_lite_block(data.call("boom", |_| -> Result<(), track_core::CoreError> { panic!("bug") })).unwrap_err();
    assert_eq!(err.code, "internal_error");
    assert!(futures_lite_block(data.settings()).is_ok(), "later calls still work");
}

fn futures_lite_block<T>(f: impl std::future::Future<Output = T>) -> T {
    gpui::block_on(f)
}

fn ai_rename(db: &mut track_core::Db, id: &str, title: &str) {
    db.edit_entries("codex", &[id.to_string()], track_core::model::EntryPatch { title: Some(Some(title.into())), ..Default::default() }).unwrap();
}

fn refresh(app: &gpui::Entity<crate::app::TrackApp>, cx: &mut gpui::VisualTestContext) {
    settle(cx);
    app.update(cx, |a, cx| a.state.update(cx, |s, cx| s.refresh_all(cx)));
    settle(cx);
}

fn seeded(cx: &mut TestAppContext, before_launch: bool) -> (gpui::Entity<crate::app::TrackApp>, &mut gpui::VisualTestContext, crate::testing::Fixture) {
    crate::testing::open_app_db(cx, move |db| {
        let t = Local::now().date_naive();
        crate::testing::insert_entry(db, "a", crate::testing::local(t, 9, 0), crate::testing::local(t, 10, 0), None, "元");
        if before_launch {
            ai_rename(db, "a", "起動前");
        }
    })
}


/// Opens the history from the status bar's flash (or the bell when the flash is gone).
fn open_history(cx: &mut gpui::VisualTestContext) {
    let target = if cx.debug_bounds("status-flash").is_some() { "status-flash" } else { "status-history" };
    let b = cx.debug_bounds(target).unwrap();
    cx.simulate_click(b.center(), gpui::Modifiers::none());
    settle(cx);
    assert!(cx.debug_bounds("status-history-list").is_some(), "the history opens");
}

fn click_sel(cx: &mut gpui::VisualTestContext, sel: &'static str) {
    let b = cx.debug_bounds(sel).unwrap_or_else(|| panic!("{sel} not rendered"));
    cx.simulate_click(b.center(), gpui::Modifiers::none());
    settle(cx);
}

/// An AI change flashes in the status bar; clicking it opens the history, where it is undone.
#[gpui::test]
fn ai_operation_flashes_and_is_undone_from_the_history(cx: &mut TestAppContext) {
    let (app, cx, f) = seeded(cx, false);
    refresh(&app, cx);
    assert!(cx.debug_bounds("status-flash").is_none());
    ai_rename(&mut f.db(), "a", "AI");
    refresh(&app, cx);
    let bar = cx.debug_bounds("status-bar").expect("status bar");
    let flash = cx.debug_bounds("status-flash").expect("the change flashes");
    assert!(bar.contains(&flash.center()), "in the status bar");
    assert!(cx.debug_bounds("status-history-undo-0").is_none(), "no undo until the history is open");
    open_history(cx);
    click_sel(cx, "status-history-undo-0");
    assert_eq!(f.db().entry("a").unwrap().unwrap().title.as_deref(), Some("元"));
    assert!(cx.debug_bounds("status-history-done-0").is_some(), "the row says it was undone");
    assert!(cx.debug_bounds("status-history-undo-0").is_none());
}

#[gpui::test]
fn operations_before_launch_are_not_shown(cx: &mut TestAppContext) {
    let (app, cx, _f) = seeded(cx, true);
    refresh(&app, cx);
    assert!(cx.debug_bounds("status-flash").is_none());
    assert!(app.read_with(cx, |a, cx| a.state.read(cx).history.items.is_empty()));
}

/// The flash goes after a few seconds; the history keeps the notice.
#[gpui::test]
fn the_flash_goes_after_a_few_seconds(cx: &mut TestAppContext) {
    let (app, cx, f) = seeded(cx, false);
    refresh(&app, cx);
    ai_rename(&mut f.db(), "a", "AI");
    refresh(&app, cx);
    assert!(cx.debug_bounds("status-flash").is_some());
    cx.executor().advance_clock(crate::app::FLASH_FOR + std::time::Duration::from_millis(100));
    settle(cx);
    assert!(cx.debug_bounds("status-flash").is_none(), "gone");
    open_history(cx);
    assert!(cx.debug_bounds("status-history-undo-0").is_some(), "still undoable from the history");
}

#[gpui::test]
fn undo_conflict_is_explained_in_the_history(cx: &mut TestAppContext) {
    let (app, cx, f) = seeded(cx, false);
    refresh(&app, cx);
    ai_rename(&mut f.db(), "a", "AI");
    refresh(&app, cx);
    f.db().update_entry("a", track_core::model::EntryPatch { title: Some(Some("手修正".into())), ..Default::default() }).unwrap();
    open_history(cx);
    click_sel(cx, "status-history-undo-0");
    assert!(cx.debug_bounds("status-history-failed-0").is_some(), "the reason is shown in the row");
    assert_eq!(f.db().entry("a").unwrap().unwrap().title.as_deref(), Some("手修正"));
}

#[gpui::test]
fn an_operation_undone_elsewhere_is_marked_done(cx: &mut TestAppContext) {
    let (app, cx, f) = seeded(cx, false);
    refresh(&app, cx);
    ai_rename(&mut f.db(), "a", "AI");
    refresh(&app, cx);
    f.db().undo_operation(None).unwrap();
    refresh(&app, cx);
    open_history(cx);
    assert!(cx.debug_bounds("status-history-done-0").is_some(), "undone from the CLI: no longer offered");
}

fn ai_record(db: &mut track_core::Db, project: &str, h: u32, title: &str) -> String {
    let t = Local::now().date_naive();
    let input = track_core::model::NewEntry { project_id: Some(project.into()), start: crate::testing::local(t, h, 0), end: crate::testing::local(t, h + 1, 0), title: Some(title.into()), ..Default::default() };
    db.op_create_entry("codex", input).unwrap().0.id
}

/// Each AI operation is a row of its own and is undone alone.
#[gpui::test]
fn each_ai_operation_is_its_own_row(cx: &mut TestAppContext) {
    let (app, cx, f) = seeded(cx, false);
    refresh(&app, cx);
    let p = f.db().projects(false).unwrap()[0].id.clone();
    let e1 = ai_record(&mut f.db(), &p, 13, "一");
    let e2 = ai_record(&mut f.db(), &p, 14, "二");
    refresh(&app, cx);
    assert_eq!(app.read_with(cx, |a, cx| a.state.read(cx).history.items.len()), 2);
    open_history(cx);
    click_sel(cx, "status-history-undo-0");
    assert!(f.db().entry(&e2).unwrap().is_none(), "the newest row undoes the newest operation");
    assert!(f.db().entry(&e1).unwrap().is_some(), "and only that one");
    assert!(cx.debug_bounds("status-history-undo-1").is_some(), "the older one is still offered");
}


#[gpui::test]
fn header_has_only_tabs_and_settings(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    let header = cx.debug_bounds("app-header").expect("header");
    for id in ["tab-calendar", "tab-reports", "app-menu"] {
        let b = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id}"));
        assert!(header.contains(&b.center()), "{id} sits in the header");
    }
    let nav = cx.debug_bounds("nav-label").expect("date label");
    assert!(nav.origin.y >= header.origin.y + header.size.height, "the date navigation stays below the header");
}

#[gpui::test]
fn header_tabs_keep_their_size_when_switching(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    let before = ["tab-calendar", "tab-reports"].map(|id| cx.debug_bounds(id).unwrap());
    cx.simulate_click(before[1].center(), gpui::Modifiers::none());
    cx.run_until_parked();
    let after = ["tab-calendar", "tab-reports"].map(|id| cx.debug_bounds(id).unwrap());
    assert_eq!(before, after, "switching moves or resizes nothing");
}

#[gpui::test]
fn header_tabs_run_down_to_the_header_edge(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    let header = cx.debug_bounds("app-header").unwrap();
    for id in ["tab-calendar", "tab-reports"] {
        let b = cx.debug_bounds(id).unwrap();
        // The underline sits on the header's bottom border.
        assert!(b.bottom() >= header.bottom() - gpui::px(1.5) && b.bottom() <= header.bottom(), "{id}: {b:?} in {header:?}");
    }
    assert!(cx.debug_bounds("header-tabs").is_none(), "no shared group background");
}

#[gpui::test]
fn a_segment_keeps_its_size_when_it_turns_off(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    let on = cx.debug_bounds("nav-today").expect("today button");
    let prev = cx.debug_bounds("nav-prev").unwrap();
    cx.simulate_click(prev.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    let off = cx.debug_bounds("nav-today").unwrap();
    assert_eq!(on.size, off.size, "today's button is the same size on and off");
}

#[gpui::test]
fn header_tab_labels_sit_on_the_header_midline(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    let settings = cx.debug_bounds("app-menu").unwrap().center().y;
    for id in ["tab-calendar-label", "tab-reports-label"] {
        let y = cx.debug_bounds(id).unwrap().center().y;
        assert!((y - settings).abs() < gpui::px(0.5), "{id}: {y:?} vs the settings button {settings:?}");
    }
}

#[gpui::test]
fn the_calendar_tab_icon_shows_todays_day(cx: &mut TestAppContext) {
    use chrono::Datelike;
    let (_app, cx) = open_app(cx);
    let day = Local::now().day();
    let mark = cx.debug_bounds(Box::leak(format!("tab-calendar-day-{day}").into_boxed_str())).expect("today's day in the icon");
    let tab = cx.debug_bounds("tab-calendar").unwrap();
    assert!(tab.contains(&mark.center()));
}

#[test]
fn the_header_redraws_just_after_midnight() {
    use chrono::NaiveDate;
    let at = |h, m, s| NaiveDate::from_ymd_opt(2026, 11, 16).unwrap().and_hms_opt(h, m, s).unwrap();
    assert_eq!(crate::app::until_next_day(at(23, 59, 0)), std::time::Duration::from_secs(61));
    assert_eq!(crate::app::until_next_day(at(0, 0, 0)), std::time::Duration::from_secs(24 * 60 * 60 + 1));
}

#[gpui::test]
fn header_icons_are_one_size(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    for id in ["tab-calendar-icon", "tab-reports-icon", "app-menu-icon"] {
        let b = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!((b.size.width, b.size.height), (crate::app::HEADER_ICON, crate::app::HEADER_ICON), "{id}");
    }
}


/// The app header keeps the Tauri version's height (44px), the traffic lights centred in it.
#[gpui::test]
fn header_is_as_tall_as_the_tauri_version(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    assert_eq!(cx.debug_bounds("app-header").expect("header").size.height, gpui::px(44.));
    let lights = crate::ui::theme::app_traffic_light_position();
    assert_eq!(lights.y * 2. + crate::ui::theme::TRAFFIC_LIGHT_HEIGHT, gpui::px(44.), "lights centred");
}

#[gpui::test]
fn apply_theme_switches_palette_and_component_theme(cx: &mut TestAppContext) {
    use crate::ui::theme::{Palette, apply_theme, palette};
    let (_app, cx) = open_app(cx);
    cx.update(|_, cx| apply_theme(true, cx));
    assert_eq!(cx.update(|_, cx| palette(cx)), Palette::dark());
    assert_eq!(cx.update(|_, cx| gpui_component::Theme::global(cx).mode), gpui_component::ThemeMode::Dark);
    cx.update(|_, cx| apply_theme(false, cx));
    assert_eq!(cx.update(|_, cx| palette(cx)), Palette::light());
    assert_eq!(cx.update(|_, cx| gpui_component::Theme::global(cx).mode), gpui_component::ThemeMode::Light);
}

/// The history keeps the newest notices first, up to HISTORY_LIMIT.
#[test]
fn notice_history_keeps_only_the_newest() {
    use crate::app::{HISTORY_LIMIT, NoticeHistory};
    let mut h = NoticeHistory::default();
    for i in 0..HISTORY_LIMIT + 5 {
        h.push(format!("n{i}"), i % 2 == 0);
    }
    assert_eq!(HISTORY_LIMIT, 10);
    assert_eq!(h.items.len(), HISTORY_LIMIT, "older ones are pushed out");
    assert_eq!(h.items[0].text, format!("n{}", HISTORY_LIMIT + 4), "newest first");
}

/// 「履歴を消す」 in the list empties it.
#[gpui::test]
fn the_history_can_be_cleared(cx: &mut TestAppContext) {
    let (app, cx, f) = seeded(cx, false);
    refresh(&app, cx);
    ai_rename(&mut f.db(), "a", "AI");
    refresh(&app, cx);
    open_history(cx);
    click_sel(cx, "status-history-clear");
    assert!(app.read_with(cx, |a, cx| a.state.read(cx).history.items.is_empty()));
    assert!(cx.debug_bounds("status-history-item-0").is_none());
    assert!(cx.debug_bounds("status-flash").is_none(), "the flash of a cleared notice goes too");
}

/// AI changes join the history too.
#[gpui::test]
fn ai_changes_are_kept_in_the_history(cx: &mut TestAppContext) {
    let (app, cx, f) = seeded(cx, false);
    refresh(&app, cx);
    ai_rename(&mut f.db(), "a", "AI");
    refresh(&app, cx);
    let items = app.read_with(cx, |a, cx| a.state.read(cx).history.items.clone());
    assert_eq!(items.len(), 1);
    assert!(items[0].text.starts_with("AI が 1 件変更しました"), "{}", items[0].text);
}

/// The status bar is about as tall as Zed's (30px), and the history panel stands right on it.
#[gpui::test]
fn status_bar_is_as_tall_as_zeds(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    let bar = cx.debug_bounds("status-bar").expect("status bar");
    assert_eq!(bar.size.height, gpui::px(30.));
    let b = cx.debug_bounds("status-history").unwrap();
    cx.simulate_click(b.center(), gpui::Modifiers::none());
    settle(cx);
    let list = cx.debug_bounds("status-history-list").expect("history");
    assert_eq!(list.bottom(), bar.top(), "the panel stands on the status bar");
}

/// The bell toggles the history: a second press closes it.
#[gpui::test]
fn the_bell_toggles_the_history(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    settle(cx);
    let b = cx.debug_bounds("status-history").unwrap();
    cx.simulate_click(b.center(), gpui::Modifiers::none());
    settle(cx);
    assert!(cx.debug_bounds("status-history-list").is_some(), "opens");
    assert_eq!(cx.debug_bounds("status-history"), Some(b), "the bell stays put");
    cx.simulate_click(b.center(), gpui::Modifiers::none());
    settle(cx);
    assert!(cx.debug_bounds("status-history-list").is_none(), "closes");
}


/// The header's 「…」 opens the app menu (as in the Tauri version) from the button.
#[gpui::test]
fn the_app_menu_lists_its_items(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    assert!(cx.debug_bounds("open-settings").is_none(), "no gear any more");
    assert!(cx.debug_bounds("app-menu-shortcuts").is_none(), "no shortcut list");
    click_sel(cx, "app-menu");
    let button = cx.debug_bounds("app-menu").unwrap();
    let items = ["app-menu-settings", "app-menu-integrations", "app-menu-update", "app-menu-about"];
    let mut last = button.bottom();
    for id in items {
        let b = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id}"));
        assert!(b.top() >= last, "{id} comes below the button, in order");
        last = b.top();
    }
}

/// 「Track について」 opens its own window (one, brought forward again), with the version and
/// a link to GitHub.
#[gpui::test]
fn about_opens_its_own_window(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    click_sel(cx, "app-menu");
    click_sel(cx, "app-menu-about");
    assert_eq!(cx.update(|_, cx| cx.windows().len()), 2, "a window of its own");
    let (handle, _) = cx.update(|_, cx| crate::ui::about::about_window(cx)).expect("about window");
    let mut acx = gpui::VisualTestContext::from_window(handle, cx);
    acx.run_until_parked();
    assert!(acx.debug_bounds("about-version").is_some(), "the version");
    let icon = acx.debug_bounds("about-icon").expect("the app icon");
    assert_eq!(icon.size.width, gpui::px(96.));
    assert!(acx.debug_bounds("about-github").is_some(), "the GitHub link");
    assert!(acx.debug_bounds("about-copyright").is_some(), "the copyright");
    cx.dispatch_action(crate::actions::ShowAbout);
    settle(cx);
    assert_eq!(cx.update(|_, cx| cx.windows().len()), 2, "the app menu's item brings the same window forward");
}

#[test]
fn about_links_to_the_repository() {
    assert_eq!(crate::ui::about::GITHUB_URL, "https://github.com/nemooon/track");
}

/// The copyright shows in 「Track について」 and, the same words, in Finder's Get Info.
#[test]
fn the_copyright_matches_the_app_bundle() {
    assert_eq!(crate::ui::about::COPYRIGHT, "© nemooon");
    let plist = include_str!("../../../packaging/Info.plist");
    assert!(plist.contains(&format!("<key>NSHumanReadableCopyright</key>\n  <string>{}</string>", crate::ui::about::COPYRIGHT)), "Info.plist carries it");
}

/// Opens 「アップデートを確認…」 with a stand-in for GitHub and Homebrew; returns the window's context.
fn open_update(cx: &mut gpui::VisualTestContext, latest: Result<&'static str, &'static str>, install: Result<(), &'static str>) -> (gpui::VisualTestContext, std::rc::Rc<std::cell::Cell<u32>>) {
    use crate::ui::update::{UpdateSource, set_source};
    use track_core::update::Release;
    let installs = std::rc::Rc::new(std::cell::Cell::new(0));
    cx.update(|_, cx| {
        set_source(
            UpdateSource {
                fetch: std::sync::Arc::new(move || latest.map(|v| Release { version: v.into(), notes: "## 変更\n- 速くなった".into(), url: "https://example.invalid".into() }).map_err(str::to_string)),
                install: std::sync::Arc::new(move |_| install.map_err(str::to_string)),
            },
            cx,
        )
    });
    click_sel(cx, "app-menu");
    click_sel(cx, "app-menu-update");
    let (handle, _) = cx.update(|_, cx| crate::ui::update::update_window(cx)).expect("update window");
    let ucx = gpui::VisualTestContext::from_window(handle, cx);
    ucx.run_until_parked();
    (ucx, installs)
}

#[gpui::test]
fn update_window_says_when_track_is_current(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    let (mut ucx, _) = open_update(cx, Ok("0.7.4"), Ok(()));
    assert!(ucx.debug_bounds("update-current").is_some(), "最新版です");
    assert!(ucx.debug_bounds("update-install").is_none());
}

#[gpui::test]
fn update_window_shows_the_notes_and_updates(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    let (mut ucx, _) = open_update(cx, Ok("9.0.0"), Ok(()));
    assert!(ucx.debug_bounds("update-notes").is_some(), "the release notes");
    let b = ucx.debug_bounds("update-install").expect("アップデート");
    ucx.simulate_click(b.center(), gpui::Modifiers::none());
    ucx.run_until_parked();
    assert!(ucx.debug_bounds("update-done").is_some(), "done");
    assert!(ucx.debug_bounds("update-restart").is_some(), "offers a restart");
}

#[gpui::test]
fn update_window_explains_a_failure(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    let (mut ucx, _) = open_update(cx, Ok("9.0.0"), Err("TrackはHomebrewでインストールされていません。"));
    let b = ucx.debug_bounds("update-install").unwrap();
    ucx.simulate_click(b.center(), gpui::Modifiers::none());
    ucx.run_until_parked();
    assert!(ucx.debug_bounds("update-error").is_some(), "the reason");
    assert!(ucx.debug_bounds("update-install").is_some(), "and a retry");
}

#[gpui::test]
fn update_window_retries_a_failed_check(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    let (mut ucx, _) = open_update(cx, Err("アップデートを確認できませんでした。"), Ok(()));
    assert!(ucx.debug_bounds("update-error").is_some());
    assert!(ucx.debug_bounds("update-retry").is_some());
}

/// TRACK_GPUI_UPDATE_PREVIEW (development builds only) stands in a sample newer release whose
/// update pretends to succeed, to look at the window without a real release.
#[test]
fn the_update_preview_offers_a_sample_release() {
    let source = crate::ui::update::preview_source("9.9.9");
    let release = (source.fetch)().unwrap();
    assert_eq!(release.version, "9.9.9");
    assert!(!release.notes.is_empty(), "with sample notes");
    assert!((source.install)(&release).is_ok(), "the update pretends to succeed");
}
