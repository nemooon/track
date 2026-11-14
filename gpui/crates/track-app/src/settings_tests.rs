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

/// Clicks `selector`, first scrolling it into view when it lies below the (1080px) test viewport.
pub fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let mut b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} not rendered"));
    let height = cx.update(|w, _| w.viewport_size().height);
    if b.top() < gpui::px(0.) || b.bottom() > height - gpui::px(20.) {
        let dy = b.center().y - height / 2.;
        cx.simulate_event(gpui::ScrollEvent {
            position: gpui::point(gpui::px(500.), gpui::px(300.)),
            delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.), -dy)),
            ..Default::default()
        });
        cx.run_until_parked();
        b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} not rendered after scrolling"));
    }
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
    assert_eq!(view.read_with(&scx, |v, _| v.section), SettingsSection::Appearance, "opens on the first section");
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

fn projects_section(cx: &mut VisualTestContext) -> (VisualTestContext, Entity<crate::settings::projects::ProjectSettings>) {
    let (scx, view) = open_section(cx, "settings-nav-projects");
    let p = view.read_with(&scx, |v, _| v.projects.clone().expect("projects section"));
    (scx, p)
}

fn tags_section(cx: &mut VisualTestContext) -> (VisualTestContext, Entity<crate::settings::tags::TagSettings>) {
    let (scx, view) = open_section(cx, "settings-nav-tags");
    let t = view.read_with(&scx, |v, _| v.tags.clone().expect("tags section"));
    (scx, t)
}

fn type_tag_name(scx: &mut VisualTestContext, t: &Entity<crate::settings::tags::TagSettings>, text: &str) {
    let input = t.read_with(scx, |t, _| t.editor.as_ref().expect("editor open").name.clone());
    scx.update(|w, cx| input.update(cx, |i, cx| i.set_value(text.to_string(), w, cx)));
}

#[gpui::test]
fn tags_are_not_on_the_projects_page(cx: &mut TestAppContext) {
    let (_app, cx, _f) = open_app_db(cx, |_| {});
    let (mut scx, _p) = projects_section(cx);
    assert!(scx.debug_bounds("tag-add").is_none());
}

#[gpui::test]
fn tag_crud(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = tags_section(cx);
    click(&mut scx, "tag-add");
    type_tag_name(&mut scx, &p, "定例");
    click(&mut scx, "editor-save");
    wait_io(&mut scx);
    assert!(scx.debug_bounds("editor-error").is_some(), "同じ名前のタグが既にあります");
    type_tag_name(&mut scx, &p, "新タグ");
    click(&mut scx, "editor-save");
    wait_io(&mut scx);
    let tag = f.db().tags().unwrap().into_iter().find(|t| t.name == "新タグ").expect("tag created");
    let edit = Box::leak(format!("tag-edit-{}", tag.id).into_boxed_str());
    click(&mut scx, edit);
    click(&mut scx, "editor-color-0");
    click(&mut scx, "editor-save");
    wait_io(&mut scx);
    assert_eq!(f.db().tags().unwrap().into_iter().find(|t| t.id == tag.id).unwrap().color, "#ef4444");
    let del = Box::leak(format!("tag-delete-{}", tag.id).into_boxed_str());
    click(&mut scx, del);
    click(&mut scx, "tag-delete-confirm");
    wait_io(&mut scx);
    assert!(f.db().tags().unwrap().iter().all(|t| t.id != tag.id));
}

fn format_editor(view: &Entity<SettingsWindow>, scx: &mut VisualTestContext) -> Entity<crate::settings::format_editor::FormatEditor> {
    view.read_with(scx, |v, cx| v.reports.as_ref().unwrap().read(cx).editing.clone().expect("format editor open"))
}

fn set_input(scx: &mut VisualTestContext, input: &Entity<gpui_component::input::InputState>, text: &str) {
    scx.update(|w, cx| input.update(cx, |i, cx| i.set_value(text.to_string(), w, cx)));
    scx.run_until_parked();
}

fn saved_formats(f: &crate::testing::Fixture) -> Vec<track_core::model::ReportCopyFormat> {
    f.db().settings().unwrap().report_copy_formats
}

#[gpui::test]
fn add_format_and_edit_columns(cx: &mut TestAppContext) {
    use track_core::model::{ReportCopyColumn, ReportCopyField, ReportDurationFormat};
    let (app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, view) = open_section(cx, "settings-nav-reports");
    assert!(scx.debug_bounds("format-row-0").is_some(), "the default 標準 format is listed");
    click(&mut scx, "format-add");
    wait_io(&mut scx);
    assert_eq!(saved_formats(&f).len(), 2, "added right away, like Tauri");
    let ed = format_editor(&view, &mut scx);
    let name = ed.read_with(&scx, |e, _| e.name.clone());
    assert_eq!(name.read_with(&scx, |n, _| n.value().to_string()), "新しい出力形式");
    set_input(&mut scx, &name, "月末報告");
    click(&mut scx, "add-column-entryCount");
    click(&mut scx, "add-column-ai");
    click(&mut scx, "column-up-4");
    click(&mut scx, "duration-format-1-decimal");
    click(&mut scx, "delimiter-comma");
    click(&mut scx, "format-save");
    wait_io(&mut scx);
    wait_io(cx);
    let saved = saved_formats(&f).into_iter().find(|x| x.name == "月末報告").expect("saved");
    assert_eq!(saved.delimiter, "comma");
    let kinds: Vec<String> = saved
        .columns
        .iter()
        .map(|c| match c {
            ReportCopyColumn::Field { field, .. } => format!("{field:?}"),
            ReportCopyColumn::Ai { label, .. } => format!("ai:{label}"),
            ReportCopyColumn::Blank { .. } => "blank".into(),
        })
        .collect();
    assert_eq!(kinds, vec!["Category", "Duration", "Percentage", "ai:AI生成項目", "EntryCount"]);
    assert!(matches!(saved.columns[1], ReportCopyColumn::Field { field: ReportCopyField::Duration, duration_format: Some(ReportDurationFormat::Decimal), .. }));
    let in_main = app.read_with(cx, |a, cx| a.state.read(cx).data.as_ref().map(|d| d.settings.report_copy_formats.iter().any(|x| x.name == "月末報告")));
    assert_eq!(in_main, Some(true), "custom output sees the new format");
}

#[gpui::test]
fn delete_format_keeps_at_least_one(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, view) = open_section(cx, "settings-nav-reports");
    click(&mut scx, "format-row-0");
    scx.run_until_parked();
    click(&mut scx, "format-delete");
    assert!(scx.debug_bounds("format-delete-confirm").is_none(), "the last format cannot be deleted");
    click(&mut scx, "format-back");
    click(&mut scx, "format-add");
    wait_io(&mut scx);
    assert_eq!(saved_formats(&f).len(), 2);
    let _ = format_editor(&view, &mut scx);
    click(&mut scx, "format-delete");
    click(&mut scx, "format-delete-confirm");
    wait_io(&mut scx);
    assert_eq!(saved_formats(&f).len(), 1);
    assert_eq!(saved_formats(&f)[0].name, "標準");
    assert!(view.read_with(&scx, |v, cx| v.reports.as_ref().unwrap().read(cx).editing.is_none()), "back to the list");
}

#[gpui::test]
fn format_validation_blocks_save(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, view) = open_section(cx, "settings-nav-reports");
    click(&mut scx, "format-row-0");
    scx.run_until_parked();
    let ed = format_editor(&view, &mut scx);
    let name = ed.read_with(&scx, |e, _| e.name.clone());
    set_input(&mut scx, &name, "  ");
    assert!(scx.debug_bounds("format-error").is_some(), "出力フォーマットの名前を入力してください");
    click(&mut scx, "format-save");
    wait_io(&mut scx);
    assert_eq!(saved_formats(&f)[0].name, "標準");
    for i in (0..3).rev() {
        let sel = Box::leak(format!("column-remove-{i}").into_boxed_str());
        click(&mut scx, sel);
    }
    set_input(&mut scx, &name, "x");
    assert!(scx.debug_bounds("format-error").is_some(), "項目を1つ以上追加してください");
}

fn ai_section(cx: &mut VisualTestContext) -> (VisualTestContext, Entity<crate::settings::ai::AiSettings>) {
    let (scx, view) = open_section(cx, "settings-nav-ai");
    let a = view.read_with(&scx, |v, _| v.ai.clone().expect("ai section"));
    (scx, a)
}

fn config_json(f: &crate::testing::Fixture) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(f.dir.join(track_core::config::FILE)).unwrap()).unwrap()
}

#[gpui::test]
fn switch_provider_and_save_config(cx: &mut TestAppContext) {
    let (app, cx, f) = open_app_db(cx, |_| {});
    std::fs::write(f.dir.join(track_core::config::FILE), r#"{"futureKey":1}"#).unwrap();
    let (mut scx, a) = ai_section(cx);
    click(&mut scx, "ai-provider-custom-command");
    let (exe, args) = a.read_with(&scx, |a, _| (a.command_executable.clone(), a.command_args.clone()));
    set_input(&mut scx, &exe, "/bin/cat");
    scx.update(|w, cx| args.update(cx, |t, cx| t.set_value("-a\n\n -b ", w, cx)));
    scx.run_until_parked();
    click(&mut scx, "ai-save");
    wait_io(&mut scx);
    wait_io(cx);
    let v = config_json(&f);
    assert_eq!(v["aiProvider"], "custom-command");
    assert_eq!(v["aiCommandExecutable"], "/bin/cat");
    assert_eq!(v["aiCommandArgs"], serde_json::json!(["-a", "-b"]));
    assert_eq!(v["futureKey"], 1, "unknown keys survive");
    assert_eq!(app.read_with(cx, |a, cx| a.state.read(cx).data.as_ref().map(|d| d.config.ai_provider.clone())), Some("custom-command".into()));
}

#[gpui::test]
fn custom_command_requires_executable(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, _a) = ai_section(cx);
    click(&mut scx, "ai-provider-custom-command");
    assert!(scx.debug_bounds("ai-error").is_some(), "実行するコマンドを指定してください");
    click(&mut scx, "ai-save");
    wait_io(&mut scx);
    assert!(!f.dir.join(track_core::config::FILE).exists() || config_json(&f)["aiProvider"] != "custom-command");
}

#[gpui::test]
fn status_reflects_saved_config(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    std::fs::write(f.dir.join(track_core::config::FILE), r#"{"aiProvider":"custom-command","aiCommandExecutable":"/bin/cat"}"#).unwrap();
    let (mut scx, a) = ai_section(cx);
    wait_io(&mut scx);
    let status = a.read_with(&scx, |a, _| a.status.clone()).expect("status loaded");
    assert!(status.available);
    assert_eq!(status.detail, "標準入力でプロンプトを渡します（/bin/cat）。");
    assert!(scx.debug_bounds("ai-status").is_some());
}

#[gpui::test]
fn codex_model_candidates_fill_the_model(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt;
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let exe = f.dir.join("codex");
    std::fs::write(
        &exe,
        "#!/bin/sh\nread a\necho '{\"id\":0,\"result\":{}}'\nread b\nread c\necho '{\"id\":1,\"result\":{\"data\":[{\"model\":\"gpt-x\",\"displayName\":\"GPT X\"}]}}'\nsleep 5\n",
    )
    .unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(f.dir.join(track_core::config::FILE), serde_json::json!({"aiProvider": "codex", "aiCodexExecutable": exe}).to_string()).unwrap();
    let (mut scx, a) = ai_section(cx);
    for _ in 0..5 {
        wait_io(&mut scx);
    }
    click(&mut scx, "codex-model-0");
    let model = a.read_with(&scx, |a, _| a.codex_model.clone());
    assert_eq!(model.read_with(&scx, |m, _| m.value().to_string()), "gpt-x");
}

fn snapshots(f: &crate::testing::Fixture) -> Vec<track_core::backup::SnapshotInfo> {
    track_core::backup::list_snapshots(&f.dir.join("exports"))
}

#[gpui::test]
fn auto_backup_runs_on_start(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    wait_io(cx);
    let s = snapshots(&f);
    assert_eq!(s.len(), 1);
    assert!(s[0].auto && s[0].name.starts_with("track-auto-"));
}

#[gpui::test]
fn backup_now_lists_snapshot(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    wait_io(cx);
    let (mut scx, _view) = open_section(cx, "settings-nav-backup");
    assert!(scx.debug_bounds("backup-row-0").is_some(), "the automatic one is listed");
    click(&mut scx, "backup-now");
    wait_io(&mut scx);
    assert_eq!(snapshots(&f).iter().filter(|s| !s.auto).count(), 1);
    assert!(scx.debug_bounds("backup-row-1").is_some());
    assert!(scx.debug_bounds("backup-latest").is_some(), "最新");
}

#[gpui::test]
fn restore_from_settings_reloads_main_window(cx: &mut TestAppContext) {
    use crate::testing::{insert_entry, local};
    let today = crate::testing::visible_weekday();
    let (app, cx, f) = open_app_db(cx, |db| insert_entry(db, "a", local(today, 10, 0), local(today, 11, 0), Some("p1"), "前"));
    wait_io(cx);
    let (mut scx, _view) = open_section(cx, "settings-nav-backup");
    click(&mut scx, "backup-now");
    wait_io(&mut scx);
    insert_entry(&f.db(), "b", local(today, 12, 0), local(today, 13, 0), Some("p1"), "後");
    click(&mut scx, "backup-restore-0");
    click(&mut scx, "backup-restore-confirm");
    wait_io(&mut scx);
    wait_io(cx);
    let titles: Vec<String> = f.db().entries(local(today, 0, 0), local(today, 23, 0)).unwrap().into_iter().filter_map(|e| e.title).collect();
    assert_eq!(titles, vec!["前".to_string()]);
    assert!(snapshots(&f).iter().filter(|s| !s.auto).count() >= 2, "the state before restoring was saved too");
    let in_main = app.read_with(cx, |a, cx| (a.calendar.read(cx).cache.get("a").is_some(), a.calendar.read(cx).cache.get("b").is_some()));
    assert_eq!(in_main, (true, false), "the calendar shows the restored data");
}

#[gpui::test]
fn export_dir_outside_dev_root_shows_error(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, view) = open_section(cx, "settings-nav-backup");
    let b = view.read_with(&scx, |v, _| v.backup.clone().expect("backup section"));
    let dir = b.read_with(&scx, |b, _| b.export_dir.clone());
    let outside = tempfile::tempdir().unwrap();
    set_input(&mut scx, &dir, &outside.path().join("bk").to_string_lossy());
    click(&mut scx, "backup-save");
    wait_io(&mut scx);
    assert!(scx.debug_bounds("backup-error").is_some());
    assert!(!outside.path().join("bk").exists());
    assert!(!f.dir.join(track_core::config::FILE).exists() || !std::fs::read_to_string(f.dir.join(track_core::config::FILE)).unwrap().contains("bk"));
}

#[gpui::test]
fn restore_resets_the_ai_operation_baseline(cx: &mut TestAppContext) {
    use crate::testing::{insert_entry, local};
    let today = crate::testing::visible_weekday();
    let (app, cx, f) = open_app_db(cx, |db| insert_entry(db, "a", local(today, 10, 0), local(today, 11, 0), None, "前"));
    wait_io(cx);
    let (mut scx, _view) = open_section(cx, "settings-nav-backup");
    click(&mut scx, "backup-now");
    wait_io(&mut scx);
    // Operations after the snapshot: ids 1..=3, the banner offers the last one.
    for t in ["x", "y", "z"] {
        f.db().edit_entries("codex", &["a".to_string()], track_core::model::EntryPatch { title: Some(Some(t.into())), ..Default::default() }).unwrap();
    }
    app.update(cx, |a, cx| a.state.update(cx, |s, cx| s.refresh_all(cx)));
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.state.read(cx).ops_seen), Some(3));
    click(&mut scx, "backup-restore-0");
    click(&mut scx, "backup-restore-confirm");
    wait_io(&mut scx);
    wait_io(cx);
    let state = app.read_with(cx, |a, cx| (a.state.read(cx).ops_seen, a.state.read(cx).ai_op.is_some()));
    assert_eq!(state, (Some(0), false), "the restored DB has no operations; nothing from before is offered");
    // The next AI operation (id 1 again) is offered.
    f.db().edit_entries("codex", &["a".to_string()], track_core::model::EntryPatch { title: Some(Some("後".into())), ..Default::default() }).unwrap();
    app.update(cx, |a, cx| a.state.update(cx, |s, cx| s.refresh_all(cx)));
    wait_io(cx);
    assert!(app.read_with(cx, |a, cx| a.state.read(cx).ai_op.is_some()));
}

#[gpui::test]
fn light_theme_is_forced_and_shared_with_settings_window(cx: &mut TestAppContext) {
    use crate::ui::theme::{Palette, palette};
    let (_app, cx, _f) = open_app_db(cx, |_| {});
    let main = cx.update(|_, cx| palette(cx));
    assert_eq!(main, Palette::light());
    assert_eq!(cx.update(|_, cx| gpui_component::Theme::global(cx).mode), gpui_component::ThemeMode::Light);
    let (mut scx, _view) = open_section(cx, "settings-nav-calendar");
    assert_eq!(scx.update(|_, cx| palette(cx)), main, "the settings window reads the same palette");
}

fn appearance_saved(f: &crate::testing::Fixture) -> String {
    config_json(f)["appearance"].as_str().unwrap_or_default().to_string()
}

#[gpui::test]
fn choosing_dark_saves_and_switches_both_windows(cx: &mut TestAppContext) {
    use crate::ui::theme::{Palette, is_dark, palette};
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, _v) = open_section(cx, "settings-nav-appearance");
    click(&mut scx, "appearance-dark");
    wait_io(&mut scx);
    wait_io(cx);
    assert_eq!(appearance_saved(&f), "dark");
    assert_eq!(cx.update(|_, cx| palette(cx)), Palette::dark(), "main window");
    assert_eq!(scx.update(|_, cx| palette(cx)), Palette::dark(), "settings window");
    click(&mut scx, "appearance-system");
    wait_io(&mut scx);
    wait_io(cx);
    let expected = if cx.update(|_, cx| is_dark(cx.window_appearance())) { Palette::dark() } else { Palette::light() };
    assert_eq!(appearance_saved(&f), "system");
    assert_eq!(cx.update(|_, cx| palette(cx)), expected);
}

#[gpui::test]
fn system_change_follows_only_when_automatic(cx: &mut TestAppContext) {
    use crate::ui::theme::{Palette, palette};
    let (app, cx, f) = open_app_db(cx, |_| {});
    f.data.save_config_blocking(track_core::config::ConfigPatch { appearance: Some(track_core::config::Appearance::Light), ..Default::default() });
    app.update(cx, |a, cx| a.on_system_appearance(true, cx));
    assert_eq!(cx.update(|_, cx| palette(cx)), Palette::light(), "fixed to light");
    f.data.save_config_blocking(track_core::config::ConfigPatch { appearance: Some(track_core::config::Appearance::System), ..Default::default() });
    app.update(cx, |a, cx| a.on_system_appearance(true, cx));
    assert_eq!(cx.update(|_, cx| palette(cx)), Palette::dark(), "automatic follows macOS");
}

#[gpui::test]
fn appearance_save_failure_keeps_theme(cx: &mut TestAppContext) {
    use crate::ui::theme::{Palette, palette};
    let (_app, cx, f) = open_app_db(cx, |_| {});
    f.data.fail_ops(&["save_config"]);
    let (mut scx, _v) = open_section(cx, "settings-nav-appearance");
    click(&mut scx, "appearance-dark");
    wait_io(&mut scx);
    assert_eq!(cx.update(|_, cx| palette(cx)), Palette::light());
    assert!(scx.debug_bounds("appearance-error").is_some());
}

#[gpui::test]
fn settings_window_follows_macos_on_its_own(cx: &mut TestAppContext) {
    use crate::ui::theme::{Palette, palette};
    let (_app, cx, _f) = open_app_db(cx, |_| {});
    let (mut scx, view) = open_section(cx, "settings-nav-calendar");
    view.update(&mut scx, |v, cx| v.on_system_appearance(true, cx));
    assert_eq!(scx.update(|_, cx| palette(cx)), Palette::dark(), "the settings window re-resolves even with the main window closed");
}

#[gpui::test]
fn settings_open_on_the_first_section(cx: &mut TestAppContext) {
    let (_app, cx, _f) = open_app_db(cx, |_| {});
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    let (mut scx, _view) = settings_cx(cx);
    assert!(scx.debug_bounds("appearance-system").is_some(), "外観 is first in the list, so it opens first");
}


// ---- プロジェクト（クライアントでまとめた一覧＋詳細） ----

fn sel(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Types into the detail's name field and presses Enter.
fn enter_name(scx: &mut VisualTestContext, p: &Entity<crate::settings::projects::ProjectSettings>, text: &str) {
    let input = p.read_with(scx, |p, _| p.name.clone().expect("name field"));
    // Typing happens in a focused field (an unfocused one shows the saved name).
    scx.update(|w, cx| input.update(cx, |i, cx| {
        i.focus(w, cx);
        i.set_value(text.to_string(), w, cx)
    }));
    scx.update(|_, cx| input.update(cx, |_, cx| cx.emit(gpui_component::input::InputEvent::PressEnter { secondary: false, shift: false })));
    wait_io(scx);
}

fn selected(scx: &mut VisualTestContext, p: &Entity<crate::settings::projects::ProjectSettings>) -> Option<crate::settings::projects::Selection> {
    p.read_with(scx, |p, _| p.selected.clone())
}

fn name_value(scx: &mut VisualTestContext, p: &Entity<crate::settings::projects::ProjectSettings>) -> String {
    p.read_with(scx, |p, cx| p.name.as_ref().map(|n| n.read(cx).value().to_string()).unwrap_or_default())
}

#[gpui::test]
fn opens_with_the_first_project_selected(cx: &mut TestAppContext) {
    use crate::settings::projects::Selection;
    let (_app, cx, _f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    assert_eq!(selected(&mut scx, &p), Some(Selection::Project("p1".into())));
    assert!(scx.debug_bounds("tree-client-c1").is_some() && scx.debug_bounds("tree-project-p1").is_some());
    assert_eq!(name_value(&mut scx, &p), "Alpha");
}

#[gpui::test]
fn add_project_under_a_client_selects_it_and_saves_a_name_on_enter(cx: &mut TestAppContext) {
    use crate::settings::projects::Selection;
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    click(&mut scx, "tree-client-add-c1");
    wait_io(&mut scx);
    let Some(Selection::Project(id)) = selected(&mut scx, &p) else { panic!("the new project is selected") };
    assert!(id != "p1" && id != "p2");
    enter_name(&mut scx, &p, "新案件");
    let pj = f.db().project(&id).unwrap();
    assert_eq!((pj.name.as_str(), pj.client_id.as_str()), ("新案件", "c1"));
    assert!(track_core::edit::PROJECT_COLORS.contains(&pj.color.as_str()));
}

#[gpui::test]
fn add_client_starts_editing_its_name(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    click(&mut scx, "tree-add-client");
    wait_io(&mut scx);
    let id = p.read_with(&scx, |p, _| p.client_edit.as_ref().map(|(id, _)| id.clone())).expect("the new client is being renamed");
    enter_client_name(&mut scx, &p, "新クライアント");
    assert_eq!(f.db().client(&id).unwrap().name, "新クライアント");
}

fn enter_client_name(scx: &mut VisualTestContext, p: &Entity<crate::settings::projects::ProjectSettings>, text: &str) {
    let input = p.read_with(scx, |p, _| p.client_edit.as_ref().map(|(_, i)| i.clone()).expect("client name field"));
    scx.update(|w, cx| input.update(cx, |i, cx| {
        i.focus(w, cx);
        i.set_value(text.to_string(), w, cx)
    }));
    scx.update(|_, cx| input.update(cx, |_, cx| cx.emit(gpui_component::input::InputEvent::PressEnter { secondary: false, shift: false })));
    wait_io(scx);
}

fn double_click(scx: &mut VisualTestContext, selector: &'static str) {
    use gpui::{MouseButton, MouseDownEvent, MouseUpEvent};
    let c = scx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} not rendered")).center();
    scx.simulate_event(MouseDownEvent { button: MouseButton::Left, position: c, modifiers: Modifiers::none(), click_count: 2, first_mouse: false });
    scx.simulate_event(MouseUpEvent { button: MouseButton::Left, position: c, modifiers: Modifiers::none(), click_count: 2 });
}

#[gpui::test]
fn double_click_renames_a_client(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    double_click(&mut scx, "tree-client-c1");
    enter_client_name(&mut scx, &p, "改名した");
    assert_eq!(f.db().client("c1").unwrap().name, "改名した");
    assert!(p.read_with(&scx, |p, _| p.client_edit.is_none()), "the field closes after saving");
}

#[gpui::test]
fn single_click_on_a_client_keeps_the_project(cx: &mut TestAppContext) {
    use crate::settings::projects::Selection;
    let (_app, cx, _f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    click(&mut scx, "tree-client-c1");
    assert!(p.read_with(&scx, |p, _| p.client_edit.is_none()));
    assert_eq!(selected(&mut scx, &p), Some(Selection::Project("p1".into())));
    assert!(scx.debug_bounds("detail-heading").is_some(), "the project form stays");
}

#[gpui::test]
fn colour_and_client_save_immediately(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |db| {
        db.conn().execute(r#"INSERT INTO "Client"(id, name, createdAt) VALUES ('c2', 'Bolt', '2026-11-01T00:00:00.000+00:00')"#, []).unwrap();
    });
    let (mut scx, p) = projects_section(cx);
    click(&mut scx, "detail-color-3");
    wait_io(&mut scx);
    assert_eq!(f.db().project("p1").unwrap().color, track_core::edit::PROJECT_COLORS[3]);
    assert!(scx.debug_bounds("detail-client").is_some(), "the client is a select");
    let select = p.read_with(&scx, |p, _| p.client_select.clone()).expect("client select");
    assert_eq!(select.read_with(&scx, |s, _| s.selected_value().cloned()), Some("c1".to_string()));
    scx.update(|_, cx| select.update(cx, |_, cx| cx.emit(gpui_component::select::SelectEvent::Confirm(Some("c2".to_string())))));
    wait_io(&mut scx);
    assert_eq!(f.db().project("p1").unwrap().client_id, "c2");
    assert_eq!(select.read_with(&scx, |s, _| s.selected_value().cloned()), Some("c2".to_string()));
}

#[gpui::test]
fn empty_name_is_not_saved(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    enter_name(&mut scx, &p, "   ");
    assert_eq!(f.db().project("p1").unwrap().name, "Alpha");
    assert_eq!(name_value(&mut scx, &p), "Alpha", "the field goes back to the saved name");
}

#[gpui::test]
fn failed_save_shows_error_and_reverts(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    f.data.fail_ops(&["update_project"]);
    enter_name(&mut scx, &p, "失敗する名前");
    assert!(scx.debug_bounds("detail-error").is_some());
    assert_eq!(f.db().project("p1").unwrap().name, "Alpha");
    assert_eq!(name_value(&mut scx, &p), "Alpha", "the field shows the saved name again");
}

#[gpui::test]
fn outside_change_keeps_the_selection(cx: &mut TestAppContext) {
    use crate::settings::projects::Selection;
    let (app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    f.db().update_project("p1", track_core::model::ProjectPatch { name: Some("外で変更".into()), ..Default::default() }).unwrap();
    let state = app.read_with(cx, |a, _| a.state.clone());
    state.update(cx, |s, cx| s.refresh_all(cx));
    wait_io(cx);
    wait_io(&mut scx);
    assert_eq!(selected(&mut scx, &p), Some(Selection::Project("p1".into())));
    assert_eq!(name_value(&mut scx, &p), "外で変更");
}

#[gpui::test]
fn archiving_a_client_moves_its_projects_to_the_archive(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, _p) = projects_section(cx);
    click(&mut scx, "tree-client-archive-c1");
    wait_io(&mut scx);
    assert!(f.db().client("c1").unwrap().archived && f.db().project("p1").unwrap().archived);
    assert!(scx.debug_bounds("tree-archived").is_some());
    // The selected project was archived with it, so the archive section opens on its own.
    assert!(scx.debug_bounds("tree-project-p1").is_some(), "shown in the open archive");
    // The same icon restores the client.
    click(&mut scx, "tree-client-archive-c1");
    wait_io(&mut scx);
    assert!(!f.db().client("c1").unwrap().archived);
}

#[gpui::test]
fn empty_client_shows_its_add_button(cx: &mut TestAppContext) {
    let (_app, cx, _f) = open_app_db(cx, |db| {
        db.conn().execute(r#"INSERT INTO "Client"(id, name, createdAt) VALUES ('c2', 'Bolt', '2026-11-01T00:00:00.000+00:00')"#, []).unwrap();
    });
    let (mut scx, _p) = projects_section(cx);
    assert!(scx.debug_bounds("tree-client-c2").is_some() && scx.debug_bounds("tree-client-add-c2").is_some());
}

#[gpui::test]
fn picker_colour_saves_and_fills_the_eyedropper(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    let picker = p.read_with(&scx, |s, _| s.picker.clone()).expect("picker state");
    let c: gpui::Hsla = gpui::rgb(0x123456).into();
    scx.update(|_, cx| picker.update(cx, |s, cx| s.set_open(true, cx)));
    scx.update(|_, cx| picker.update(cx, |_, cx| cx.emit(gpui_component::color_picker::ColorPickerEvent::Change(Some(c)))));
    scx.update(|_, cx| picker.update(cx, |s, cx| s.set_open(false, cx)));
    wait_io(&mut scx);
    assert_eq!(f.db().project("p1").unwrap().color, "#123456");
    assert!(scx.debug_bounds("editor-color-custom").is_some());
    let swatch = scx.debug_bounds("detail-color-0").expect("swatch").size;
    assert_eq!(scx.debug_bounds("editor-color-custom").unwrap().size, swatch, "same circle as the presets");
    click(&mut scx, "editor-color-custom");
    for id in ["picker-hue", "picker-saturation", "picker-lightness", "picker-hex", "picker-hue-track"] {
        assert!(scx.debug_bounds(sel(id.to_string())).is_some(), "{id}");
    }
}

#[gpui::test]
fn switching_selection_saves_the_typed_name(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    let input = p.read_with(&scx, |p, _| p.name.clone().expect("name field"));
    scx.update(|w, cx| input.update(cx, |i, cx| {
        i.focus(w, cx);
        i.set_value("入力途中".to_string(), w, cx)
    }));
    // Clicking another row does not blur the field in GPUI.
    click(&mut scx, "tree-project-p2");
    wait_io(&mut scx);
    assert_eq!(f.db().project("p1").unwrap().name, "入力途中");
}

#[gpui::test]
fn outside_rename_survives_a_focused_untouched_field(cx: &mut TestAppContext) {
    let (app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    let input = p.read_with(&scx, |p, _| p.name.clone().expect("name field"));
    scx.update(|w, cx| input.update(cx, |i, cx| i.focus(w, cx)));
    f.db().update_project("p1", track_core::model::ProjectPatch { name: Some("外で変更".into()), ..Default::default() }).unwrap();
    let state = app.read_with(cx, |a, _| a.state.clone());
    state.update(cx, |s, cx| s.refresh_all(cx));
    wait_io(cx);
    wait_io(&mut scx);
    assert_eq!(name_value(&mut scx, &p), "外で変更", "an untouched field follows the saved name");
    scx.update(|_, cx| input.update(cx, |_, cx| cx.emit(gpui_component::input::InputEvent::Blur)));
    wait_io(&mut scx);
    assert_eq!(f.db().project("p1").unwrap().name, "外で変更");
}

#[gpui::test]
fn picker_saves_when_closed_not_while_dragging(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    let before = f.db().project("p1").unwrap().color;
    let picker = p.read_with(&scx, |s, _| s.picker.clone()).expect("picker state");
    scx.update(|_, cx| picker.update(cx, |s, cx| s.set_open(true, cx)));
    for v in [0x111111, 0x123456] {
        let c: gpui::Hsla = gpui::rgb(v).into();
        scx.update(|_, cx| picker.update(cx, |_, cx| cx.emit(gpui_component::color_picker::ColorPickerEvent::Change(Some(c)))));
    }
    wait_io(&mut scx);
    assert_eq!(f.db().project("p1").unwrap().color, before, "nothing saved while the popover is open");
    scx.update(|_, cx| picker.update(cx, |s, cx| s.set_open(false, cx)));
    wait_io(&mut scx);
    assert_eq!(f.db().project("p1").unwrap().color, "#123456");
}

#[gpui::test]
fn name_field_waits_for_the_selected_item(cx: &mut TestAppContext) {
    use crate::settings::projects::Selection;
    let (_app, cx, _f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    // Just added: selected before the re-read that brings it in.
    p.update(&mut scx, |p, cx| {
        p.selected = Some(Selection::Project("not-yet-read".into()));
        cx.notify();
    });
    scx.run_until_parked();
    assert!(p.read_with(&scx, |p, _| p.name.is_none()), "no empty field before the item is read");
}

#[gpui::test]
fn failed_colour_save_keeps_the_typed_name(cx: &mut TestAppContext) {
    let (_app, cx, f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    let input = p.read_with(&scx, |p, _| p.name.clone().expect("name field"));
    scx.update(|w, cx| input.update(cx, |i, cx| {
        i.focus(w, cx);
        i.set_value("入力途中".to_string(), w, cx)
    }));
    f.data.fail_ops(&["update_project"]);
    click(&mut scx, "detail-color-3");
    wait_io(&mut scx);
    assert!(scx.debug_bounds("detail-error").is_some());
    assert_eq!(name_value(&mut scx, &p), "入力途中");
}

#[gpui::test]
fn archived_client_has_no_add_button(cx: &mut TestAppContext) {
    let (_app, cx, _f) = open_app_db(cx, |db| {
        db.conn().execute(r#"INSERT INTO "Client"(id, name, archived, createdAt) VALUES ('c2', 'Old', 1, '2026-11-01T00:00:00.000+00:00')"#, []).unwrap();
        db.conn().execute(r##"INSERT INTO "Project"(id, clientId, name, color, archived, createdAt) VALUES ('p3', 'c2', 'Gone', '#3b82f6', 1, '2026-11-01T00:00:00.000+00:00')"##, []).unwrap();
    });
    let (mut scx, _p) = projects_section(cx);
    click(&mut scx, "tree-archived");
    assert!(scx.debug_bounds("tree-client-c2").is_some(), "the archived client is listed");
    assert!(scx.debug_bounds("tree-client-add-c2").is_none());
}

#[gpui::test]
fn picker_follows_a_preset_colour(cx: &mut TestAppContext) {
    let (_app, cx, _f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    click(&mut scx, "detail-color-3");
    wait_io(&mut scx);
    let picker = p.read_with(&scx, |s, _| s.picker.clone()).expect("picker state");
    let v = picker.read_with(&scx, |s, _| s.value()).expect("a colour");
    assert_eq!(crate::ui::theme::to_hex(v), track_core::edit::PROJECT_COLORS[3]);
}

#[gpui::test]
fn a_save_reads_once(cx: &mut TestAppContext) {
    let (_app, cx, _f) = open_app_db(cx, |_| {});
    let (mut scx, p) = projects_section(cx);
    let before = p.read_with(&scx, |p, _| p.reloads);
    click(&mut scx, "detail-color-3");
    wait_io(&mut scx);
    assert_eq!(p.read_with(&scx, |p, _| p.reloads) - before, 1);
}
