//! Settings window (headless).
use crate::settings::{SettingsSection, SettingsWindow, settings_window};
use crate::testing::{open_app, open_app_db, wait_io};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use track_core::model::SettingsPatch;

fn window_count(cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| cx.windows().len())
}

/// A test context bound to the open settings window, and its view.
pub fn settings_cx(cx: &mut VisualTestContext) -> (VisualTestContext, Entity<SettingsWindow>) {
    let (handle, view) = cx.update(|_, cx| settings_window(cx)).expect("settings window is open");
    (VisualTestContext::from_window(handle, cx), view)
}

pub fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} not rendered"));
    cx.simulate_click(b.center(), Modifiers::none());
}

#[gpui::test]
fn cmd_comma_opens_single_settings_window(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    assert_eq!(window_count(cx), 1);
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    assert_eq!(window_count(cx), 2);
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    assert_eq!(window_count(cx), 2, "a second ⌘, brings the same window forward");
}

#[gpui::test]
fn header_button_opens_settings(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    click(cx, "open-settings");
    cx.run_until_parked();
    assert_eq!(window_count(cx), 2);
}

#[gpui::test]
fn sidebar_switches_sections(cx: &mut TestAppContext) {
    let (_app, cx) = open_app(cx);
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    let (mut scx, view) = settings_cx(cx);
    assert_eq!(view.read_with(&scx, |v, _| v.section), SettingsSection::Calendar);
    for (sel, section) in [
        ("settings-nav-projects", SettingsSection::Projects),
        ("settings-nav-ai", SettingsSection::Ai),
        ("settings-nav-backup", SettingsSection::Backup),
        ("settings-nav-reports", SettingsSection::Reports),
    ] {
        click(&mut scx, sel);
        assert_eq!(view.read_with(&scx, |v, _| v.section), section);
    }
    assert!(scx.debug_bounds("settings-nav-notes").is_none(), "no memo section");
}

#[gpui::test]
fn saving_settings_refreshes_main_window(cx: &mut TestAppContext) {
    let (app, cx, _f) = open_app_db(cx, |_| {});
    let state = app.read_with(cx, |a, _| a.state.clone());
    state.update(cx, |s, cx| s.save_settings(SettingsPatch { work_start: Some(540), ..Default::default() }, cx)).detach();
    wait_io(cx);
    assert_eq!(state.read_with(cx, |s, _| s.data.as_ref().map(|d| d.settings.work_start)), Some(540));
    assert_eq!(state.read_with(cx, |s, _| s.reloads), 1, "pages were told to reload");
}

fn open_section(cx: &mut VisualTestContext, nav: &'static str) -> (VisualTestContext, Entity<SettingsWindow>) {
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    let (mut scx, view) = settings_cx(cx);
    click(&mut scx, nav);
    wait_io(&mut scx);
    (scx, view)
}

fn db_settings(f: &crate::testing::Fixture) -> track_core::model::Settings {
    f.db().settings().unwrap()
}

#[gpui::test]
fn work_schedule_save_and_discard(cx: &mut TestAppContext) {
    let (app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, view) = open_section(cx, "settings-nav-calendar");
    let cal = view.read_with(&scx, |v, _| v.calendar.clone().expect("calendar section"));
    assert_eq!(cal.read_with(&scx, |c, cx| (c.work_start, c.work_end, c.dirty_in(cx))), (600, 1110, false));
    assert!(scx.debug_bounds("work-saved").is_some(), "保存済み");
    click(&mut scx, "work-start-inc");
    assert_eq!(cal.read_with(&scx, |c, cx| (c.work_start, c.dirty_in(cx))), (630, true));
    click(&mut scx, "work-discard");
    assert_eq!(cal.read_with(&scx, |c, cx| (c.work_start, c.dirty_in(cx))), (600, false));
    click(&mut scx, "work-start-dec");
    click(&mut scx, "work-day-6");
    click(&mut scx, "work-save");
    wait_io(&mut scx);
    let s = db_settings(&f);
    assert_eq!((s.work_start, s.work_days), (570, vec![1, 2, 3, 4, 5, 6]));
    assert_eq!(app.read_with(cx, |a, cx| a.state.read(cx).data.as_ref().map(|d| d.settings.work_start)), Some(570), "main window refreshed");
    assert!(!cal.read_with(&scx, |c, cx| c.dirty_in(cx)));
}

#[gpui::test]
fn invalid_schedule_shows_error_and_keeps_db(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, view) = open_section(cx, "settings-nav-calendar");
    let cal = view.read_with(&scx, |v, _| v.calendar.clone().unwrap());
    for d in ["work-day-1", "work-day-2", "work-day-3", "work-day-4", "work-day-5"] {
        click(&mut scx, d);
    }
    assert_eq!(cal.read_with(&scx, |c, _| c.validation()), Some("勤務日を1日以上選択してください"));
    assert!(scx.debug_bounds("work-error").is_some());
    click(&mut scx, "work-save");
    wait_io(&mut scx);
    assert_eq!(db_settings(&f).work_days, vec![1, 2, 3, 4, 5], "nothing saved");
}

#[gpui::test]
fn weekly_template_saves(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, view) = open_section(cx, "settings-nav-reports");
    let rep = view.read_with(&scx, |v, _| v.reports.clone().expect("reports section"));
    let editor = rep.read_with(&scx, |r, _| r.template.clone());
    assert_eq!(editor.read_with(&scx, |e, _| e.value().to_string()), "{{期間}} {{合計時間}}");
    scx.update(|w, cx| editor.update(cx, |e, cx| e.set_value("  ", w, cx)));
    scx.run_until_parked();
    assert!(scx.debug_bounds("template-error").is_some());
    scx.update(|w, cx| editor.update(cx, |e, cx| e.set_value("## 今週\n{{期間}}", w, cx)));
    scx.run_until_parked();
    click(&mut scx, "template-save");
    wait_io(&mut scx);
    assert_eq!(db_settings(&f).weekly_report_template, "## 今週\n{{期間}}");
}

#[gpui::test]
fn calendar_draft_follows_saved_values_until_edited(cx: &mut TestAppContext) {
    let (app, cx, _f) = open_app_db(cx, |_| {});
    let (mut scx, view) = open_section(cx, "settings-nav-calendar");
    let cal = view.read_with(&scx, |v, _| v.calendar.clone().unwrap());
    let state = app.read_with(cx, |a, _| a.state.clone());
    state.update(cx, |s, cx| s.save_settings(SettingsPatch { work_start: Some(540), ..Default::default() }, cx)).detach();
    wait_io(cx);
    assert_eq!(cal.read_with(&scx, |c, _| c.work_start), 540, "untouched draft shows the newly saved value");
    click(&mut scx, "work-end-dec");
    state.update(cx, |s, cx| s.save_settings(SettingsPatch { work_start: Some(480), ..Default::default() }, cx)).detach();
    wait_io(cx);
    assert_eq!(cal.read_with(&scx, |c, _| (c.work_start, c.work_end)), (540, 1080), "an edited draft is kept");
}

#[gpui::test]
fn template_follows_saved_value_until_edited(cx: &mut TestAppContext) {
    let (app, cx, _f) = open_app_db(cx, |_| {});
    let (scx, view) = open_section(cx, "settings-nav-reports");
    let editor = view.read_with(&scx, |v, cx| v.reports.as_ref().unwrap().read(cx).template.clone());
    let state = app.read_with(cx, |a, _| a.state.clone());
    state.update(cx, |s, cx| s.save_settings(SettingsPatch { weekly_report_template: Some("新しい".into()), ..Default::default() }, cx)).detach();
    wait_io(cx);
    scx.run_until_parked();
    assert_eq!(editor.read_with(&scx, |e, _| e.value().to_string()), "新しい");
}
