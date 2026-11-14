use crate::reports::Children;
use crate::testing::{Fixture, fake_ai, insert_entry, local, open_app_db, wait_io};
use chrono::Local;
use gpui::{Modifiers, TestAppContext};
use track_core::Db;
use track_core::model::GroupBy;

/// This week: Alpha 「定例」 30 + 60 min (e1, e2) and one 30 min entry without a project (n1).
fn seed_week(db: &mut Db) {
    let today = Local::now().date_naive();
    insert_entry(db, "e1", local(today, 0, 0), local(today, 0, 30), Some("p1"), "定例");
    insert_entry(db, "e2", local(today, 1, 0), local(today, 2, 0), Some("p1"), "定例");
    insert_entry(db, "n1", local(today, 3, 0), local(today, 3, 30), None, "メモ整理");
}

fn setup(cx: &mut TestAppContext) -> (gpui::Entity<crate::app::TrackApp>, &mut gpui::VisualTestContext, Fixture) {
    let (app, cx, f) = open_app_db(cx, seed_week);
    cx.simulate_keystrokes("cmd-2");
    wait_io(cx);
    (app, cx, f)
}

fn click(cx: &mut gpui::VisualTestContext, selector: &'static str) {
    let b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} not rendered"));
    cx.simulate_click(b.center(), Modifiers::none());
}

#[gpui::test]
fn loads_report_and_expands_row(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let rows = app.read_with(cx, |a, cx| a.reports.read(cx).data.as_ref().map(|d| d.rows.iter().map(|r| (r.key.clone(), r.total_minutes)).collect::<Vec<_>>()));
    assert_eq!(rows, Some(vec![("p1".to_string(), 90), ("__none__".to_string(), 30)]));
    click(cx, "row-p1");
    wait_io(cx);
    let loaded = app.read_with(cx, |a, cx| a.reports.read(cx).children.get("project:p1").cloned());
    assert!(matches!(loaded, Some(Children::Loaded(ref v)) if v.len() == 2), "{loaded:?}");
    // expanded state is persisted per groupBy
    assert_eq!(app.read_with(cx, |a, cx| a.state.read(cx).persist.reports_expanded.get("project").cloned()), Some(vec!["project:p1".to_string()]));
}

#[gpui::test]
fn toggle_all_expands_every_row_then_collapses(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    click(cx, "toggle-all");
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).expanded.len()), 2);
    click(cx, "toggle-all");
    assert!(app.read_with(cx, |a, cx| a.reports.read(cx).expanded.is_empty()));
}

#[gpui::test]
fn project_filter_requery_and_client_change_drops_projects(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    click(cx, "filter-プロジェクト");
    click(cx, "filter-option-p2");
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).project_ids.clone()), vec!["p2".to_string()]);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).data.as_ref().map(|d| d.rows.len())), Some(0), "Beta has no entries");
    app.update(cx, |a, cx| a.reports.update(cx, |r, cx| r.set_selected(crate::reports::FilterKind::Client, vec!["zz".into()], cx)));
    assert!(app.read_with(cx, |a, cx| a.reports.read(cx).project_ids.is_empty()));
}

#[gpui::test]
fn group_by_switch_and_same_titles_toggle(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    app.update(cx, |a, cx| a.reports.update(cx, |r, cx| r.set_group_by(GroupBy::Tag, cx)));
    wait_io(cx);
    let keys = app.read_with(cx, |a, cx| a.reports.read(cx).data.as_ref().map(|d| d.rows.iter().map(|r| r.key.clone()).collect::<Vec<_>>()));
    assert_eq!(keys, Some(vec!["t1".to_string(), "__none__".to_string()]));
    click(cx, "group-same-titles");
    assert!(app.read_with(cx, |a, cx| a.state.read(cx).persist.group_same_titles));
}

fn ai_setup<'a>(cx: &'a mut TestAppContext, ai_text: &'static str) -> (gpui::Entity<crate::app::TrackApp>, &'a mut gpui::VisualTestContext, Fixture) {
    let formats = serde_json::json!([
        {"id": "plain", "name": "標準", "target": "ai-aggregation", "delimiter": "tab", "includeHeader": true, "aiPrompt": "",
         "columns": [{"id": "c", "kind": "field", "field": "category"}, {"id": "n", "kind": "field", "field": "entryCount"}]},
        {"id": "ai", "name": "AI", "target": "ai-aggregation", "delimiter": "comma", "includeHeader": false, "aiPrompt": "目的別",
         "columns": [{"id": "c", "kind": "field", "field": "category"}, {"id": "s", "kind": "field", "field": "summary"}]}
    ])
    .to_string();
    let (app, cx, f) = open_app_db(
        cx,
        move |db| {
            seed_week(db);
            db.conn()
                .execute(r#"UPDATE "Settings" SET reportCopyFormats = ?1, weeklyReportTemplate = ?2"#, [formats.as_str(), "## 主な作業\n（ここ）\n\n## 課題\n- {{期間}}"])
                .unwrap();
        },
    );
    fake_ai(&f.dir, ai_text);
    cx.simulate_keystrokes("cmd-2");
    wait_io(cx);
    (app, cx, f)
}

fn output(app: &gpui::Entity<crate::app::TrackApp>, cx: &mut gpui::VisualTestContext) -> gpui::Entity<crate::reports::CustomOutputView> {
    app.read_with(cx, |a, cx| a.reports.read(cx).output.clone().expect("output dialog"))
}

#[gpui::test]
fn custom_output_without_ai_uses_report_rows_and_copies_tsv(cx: &mut TestAppContext) {
    let (app, cx, _f) = ai_setup(cx, "");
    click(cx, "output-open");
    wait_io(cx);
    click(cx, "output-create");
    wait_io(cx);
    let out = output(&app, cx);
    let rows = out.read_with(cx, |o, _| o.result.clone());
    assert_eq!(rows.iter().map(|r| (r.label.as_str(), r.entry_count)).collect::<Vec<_>>(), vec![("Alpha", 2), ("プロジェクトなし", 1)]);
    click(cx, "output-copy");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()), Some("集計名\t件数\nAlpha\t2\nプロジェクトなし\t1".into()));
    assert!(out.read_with(cx, |o, _| o.copied));
}

#[gpui::test]
fn custom_output_with_ai_streams_and_restores_from_cache(cx: &mut TestAppContext) {
    let (app, cx, f) = ai_setup(cx, r#"{"groups":[{"label":"定例","summary":"会議","entryIds":["e1","e2"]}]}"#);
    click(cx, "output-open");
    wait_io(cx);
    click(cx, "format-select");
    click(cx, "format-ai");
    click(cx, "output-create");
    wait_io(cx);
    let out = output(&app, cx);
    let rows = out.read_with(cx, |o, _| o.result.clone());
    assert_eq!((rows[0].label.as_str(), rows[0].minutes, rows[0].summary.as_str()), ("定例", 90, "会議"));
    // The configured command received the aggregation prompt on stdin.
    let prompt = std::fs::read_to_string(f.dir.join("ai-input.txt")).expect("ai command ran");
    assert!(prompt.starts_with("入力には、工数記録と集計の観点が含まれます。"));
    assert!(prompt.contains("目的別"));
    // Re-open: the cached result for the same format and data is restored.
    cx.update(gpui_component::WindowExt::close_dialog);
    click(cx, "output-open");
    wait_io(cx);
    click(cx, "format-select");
    click(cx, "format-ai");
    wait_io(cx);
    let out = output(&app, cx);
    assert!(out.read_with(cx, |o, _| o.restored_at.is_some()));
    assert_eq!(out.read_with(cx, |o, _| o.result.len()), 2, "定例 + その他");
}

#[gpui::test]
fn weekly_report_replaces_main_work_with_fixed_text(cx: &mut TestAppContext) {
    let (app, cx, _f) = ai_setup(cx, "## 主な作業\n- AIの文章\n\n## 課題\n- なし");
    click(cx, "weekly-open");
    wait_io(cx);
    let weekly = app.read_with(cx, |a, cx| a.reports.read(cx).weekly.clone().unwrap());
    let text = weekly.read_with(cx, |w, cx| w.editor.read(cx).value().to_string());
    assert_eq!(text, "## 主な作業\n\n- **Acme / Alpha**（1時間30分）：定例\n- **プロジェクトなし**（30分）：メモ整理\n\n## 課題\n- なし");
    click(cx, "weekly-copy");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()), Some(text));
}

#[gpui::test]
fn weekly_report_requires_template(cx: &mut TestAppContext) {
    let (app, cx, _f) = ai_setup(cx, "x");
    app.update(cx, |a, cx| a.state.update(cx, |s, _| s.data.as_mut().unwrap().settings.weekly_report_template = "  ".into()));
    click(cx, "weekly-open");
    wait_io(cx);
    let weekly = app.read_with(cx, |a, cx| a.reports.read(cx).weekly.clone().unwrap());
    assert_eq!(weekly.read_with(cx, |w, _| w.error.clone()), Some("設定の「レポート」で出力テンプレートを保存してください。".into()));
}

#[gpui::test]
fn weekly_copy_button_writes_clipboard(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    // Grouped by tag on screen; the copy is still per project.
    app.update(cx, |a, cx| a.reports.update(cx, |r, cx| r.set_group_by(GroupBy::Tag, cx)));
    wait_io(cx);
    click(cx, "weekly-copy-totals");
    wait_io(cx);
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()), Some("Alpha\t1.5\nプロジェクトなし\t0.5".into()));
}

#[gpui::test]
fn duration_style_defaults_to_decimal_and_toggles(cx: &mut TestAppContext) {
    use crate::persist::DurationStyle;
    let (app, cx, _f) = setup(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).duration(90, cx)), "1.5h");
    click(cx, "duration-hm");
    assert_eq!(app.read_with(cx, |a, cx| a.state.read(cx).persist.duration_style), DurationStyle::HoursMinutes);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).duration(90, cx)), "1h 30m");
    click(cx, "duration-decimal");
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).duration(90, cx)), "1.5h");
}

fn row_keys(app: &gpui::Entity<crate::app::TrackApp>, cx: &mut gpui::VisualTestContext) -> Vec<String> {
    app.read_with(cx, |a, cx| a.reports.read(cx).data.as_ref().map(|d| d.rows.iter().map(|r| r.key.clone()).collect()).unwrap_or_default())
}

#[gpui::test]
fn summary_column_is_gone(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    assert!(cx.debug_bounds("donut").is_none() && cx.debug_bounds("summary-toggle").is_none());
    let key = row_keys(&app, cx)[0].clone();
    let sel: &'static str = Box::leak(format!("row-{key}").into_boxed_str());
    let row = cx.debug_bounds(sel).expect("row");
    assert!(row.origin.x <= gpui::px(16.), "the table spans the full width: {row:?}");
}

#[gpui::test]
fn share_is_a_percentage_without_a_bar(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let keys = row_keys(&app, cx);
    assert_eq!(keys.len(), 2, "{keys:?}");
    for key in &keys {
        assert!(cx.debug_bounds(leak(format!("share-bar-{key}"))).is_none(), "no bar for {key}");
        assert!(cx.debug_bounds(leak(format!("share-{key}"))).is_some(), "a percentage for {key}");
    }
}

#[gpui::test]
fn reports_status_bar_shows_row_count_not_a_second_total(cx: &mut TestAppContext) {
    let (_app, cx, _f) = setup(cx);
    let bar = cx.debug_bounds("status-bar").expect("status bar");
    let rows = cx.debug_bounds("status-rows").expect("row count");
    assert!(bar.contains(&rows.center()));
    assert!(cx.debug_bounds("status-total").is_none(), "the table's 合計 row already shows the total");
}

#[gpui::test]
fn status_bar_hides_row_count_while_loading(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    assert!(cx.debug_bounds("status-rows").is_some());
    app.update(cx, |a, cx| a.reports.update(cx, |r, cx| {
        r.loading = true;
        cx.notify();
    }));
    cx.run_until_parked();
    assert!(cx.debug_bounds("status-rows").is_none(), "no stale count from the previous period");
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[gpui::test]
fn heatmap_shows_a_year_between_the_filters_and_the_table(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let days = app.read_with(cx, |a, cx| a.reports.read(cx).heatmap.clone()).expect("heatmap loaded");
    assert_eq!(days.len(), 53 * 7, "53 weeks, like GitHub's year");
    let today = Local::now().date_naive();
    assert_eq!(days.iter().find(|(d, _)| *d == today).map(|(_, m)| *m), Some(120), "today's 2h");
    assert!(cx.debug_bounds(leak(format!("heat-{today}"))).is_some());
    let heat = cx.debug_bounds("report-heatmap").expect("heatmap");
    assert!(heat.bottom() <= cx.debug_bounds("row-p1").expect("table row").top(), "above the table");
    assert!(heat.size.width > gpui::px(600.), "full width");
}

#[gpui::test]
fn clicking_a_heatmap_day_moves_to_its_period(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let first = app.read_with(cx, |a, cx| a.reports.read(cx).heatmap.clone()).unwrap()[0].0;
    click(cx, leak(format!("heat-{first}")));
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).anchor), first);
}

#[test]
fn heatmap_levels_by_hours() {
    use crate::reports::heatmap::level;
    assert_eq!([level(0), level(1), level(119), level(120), level(239), level(240), level(359), level(360), level(600)], [0, 1, 1, 2, 2, 3, 3, 4, 4]);
}

#[gpui::test]
fn heatmap_cells_are_square(cx: &mut TestAppContext) {
    let (_app, cx, _f) = setup(cx);
    let today = Local::now().date_naive();
    let cell = cx.debug_bounds(leak(format!("heat-{today}"))).expect("today's cell").size;
    assert!((cell.width - cell.height).abs() < gpui::px(0.5), "{cell:?}");
}

#[test]
fn heatmap_labels_the_week_that_holds_a_months_first_day() {
    use crate::reports::heatmap::month_label;
    let d = |m, day| chrono::NaiveDate::from_ymd_opt(2026, m, day).unwrap();
    assert_eq!(month_label(d(11, 1)), Some("11月".to_string()), "starts on the 1st");
    assert_eq!(month_label(d(11, 29)), Some("12月".to_string()), "holds 12/1");
    assert_eq!(month_label(d(11, 8)), None);
}

#[test]
fn a_heatmap_day_picks_its_week_or_month() {
    use crate::reports::heatmap::pick_range;
    use track_core::model::ReportRange;
    let d = |m, day| chrono::NaiveDate::from_ymd_opt(2026, m, day).unwrap();
    assert_eq!(pick_range(ReportRange::Week, d(11, 11)), (d(11, 8), d(11, 15)), "Sunday-start week");
    assert_eq!(pick_range(ReportRange::Month, d(11, 11)), (d(11, 1), d(12, 1)));
}

#[gpui::test]
fn hovering_a_heatmap_day_marks_what_a_click_would_pick(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let first = app.read_with(cx, |a, cx| a.reports.read(cx).heatmap.clone()).unwrap()[0].0;
    let b = cx.debug_bounds(leak(format!("heat-{first}"))).expect("cell");
    cx.simulate_mouse_move(b.center(), None, Modifiers::none());
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).heat_hover), Some(first));
    cx.simulate_mouse_move(gpui::point(gpui::px(5.), gpui::px(5.)), None, Modifiers::none());
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).heat_hover), None, "leaving the cell clears it");
}

#[gpui::test]
fn the_gap_beside_a_heatmap_cell_clicks_that_cell(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let first = app.read_with(cx, |a, cx| a.reports.read(cx).heatmap.clone()).unwrap()[0].0;
    let cell = cx.debug_bounds(leak(format!("heat-{first}"))).expect("cell");
    // Just right of the square, in the gap before the next week's cell.
    let gap = gpui::point(cell.right() + gpui::px(1.), cell.center().y);
    cx.simulate_click(gap, Modifiers::none());
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).anchor), first);
}

#[gpui::test]
fn moving_period_remembers_the_one_to_fade_from(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    let shown = app.read_with(cx, |a, cx| a.reports.read(cx).heat_shown);
    assert!(shown.is_some(), "the shown period is remembered");
    let first = app.read_with(cx, |a, cx| a.reports.read(cx).heatmap.clone()).unwrap()[0].0;
    click(cx, leak(format!("heat-{first}")));
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).heat_from), shown, "cells fade from the previous period");
}

#[gpui::test]
fn the_heatmap_slides_by_the_weeks_it_moved(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).heat_shift), 0, "nothing to slide at first");
    let first = app.read_with(cx, |a, cx| a.reports.read(cx).heatmap.clone()).unwrap()[0].0;
    click(cx, leak(format!("heat-{first}")));
    wait_io(cx);
    // The first column's week is now the middle one: the range moved 26 weeks back.
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).heat_shift), -26);
    let now_first = app.read_with(cx, |a, cx| a.reports.read(cx).heatmap.clone()).unwrap()[0].0;
    assert_eq!((first - now_first).num_weeks(), 26);
}

#[test]
fn the_period_is_strong_and_the_hovered_range_in_between() {
    use crate::reports::heatmap::cell_opacity;
    assert_eq!(cell_opacity(true, false), 1.0, "the report's period");
    assert_eq!(cell_opacity(true, true), 1.0, "hovering the period changes nothing");
    let (hover, faded) = (cell_opacity(false, true), cell_opacity(false, false));
    assert!(faded < hover && hover < 1.0, "hover {hover} sits between faded {faded} and the period");
}

#[gpui::test]
fn moving_the_period_keeps_the_table_until_the_new_one_is_ready(cx: &mut TestAppContext) {
    let (app, cx, _f) = setup(cx);
    click(cx, "row-p1");
    wait_io(cx);
    app.update(cx, |a, cx| {
        a.reports.update(cx, |r, cx| {
            r.anchor -= chrono::Duration::days(7);
            r.ensure_loaded(cx);
            assert!(r.data.is_some(), "the old table stays while the new one loads");
            assert!(matches!(r.children.get("project:p1"), Some(Children::Loaded(_))), "and so do the open row's entries");
        })
    });
    wait_io(cx);
    let (rows, child) = app.read_with(cx, |a, cx| {
        let r = a.reports.read(cx);
        (r.data.as_ref().map(|d| d.rows.len()), r.children.get("project:p1").cloned())
    });
    assert_eq!(rows, Some(0), "last week has no entries");
    assert!(child.is_none() || matches!(child, Some(Children::Loaded(_))), "never left loading: {child:?}");
}
