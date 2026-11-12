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
