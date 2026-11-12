use crate::reports::Children;
use crate::testing::{Fixture, insert_entry, local, open_app_db, stub_server, wait_io};
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
    let (app, cx, f) = open_app_db(cx, seed_week, "http://127.0.0.1:9/api");
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

fn ai_setup<'a>(cx: &'a mut TestAppContext, ai_text: &'static str) -> (gpui::Entity<crate::app::TrackApp>, &'a mut gpui::VisualTestContext, std::sync::Arc<std::sync::Mutex<Vec<crate::testing::Recorded>>>) {
    let (base, log) = stub_server(move |_, path| match path {
        "/api/ai/generate-stream" => {
            let result = serde_json::json!({"type": "result", "text": ai_text, "provider": "codex"}).to_string();
            (200, format!("{{\"type\":\"progress\",\"kind\":\"reasoning\",\"message\":\"分類中\"}}\n{{\"type\":\"heartbeat\"}}\n{result}\n"))
        }
        _ => (404, r#"{"error":"nf"}"#.into()),
    });
    let formats = serde_json::json!([
        {"id": "plain", "name": "標準", "target": "ai-aggregation", "delimiter": "tab", "includeHeader": true, "aiPrompt": "",
         "columns": [{"id": "c", "kind": "field", "field": "category"}, {"id": "n", "kind": "field", "field": "entryCount"}]},
        {"id": "ai", "name": "AI", "target": "ai-aggregation", "delimiter": "comma", "includeHeader": false, "aiPrompt": "目的別",
         "columns": [{"id": "c", "kind": "field", "field": "category"}, {"id": "s", "kind": "field", "field": "summary"}]}
    ])
    .to_string();
    let (app, cx, _f) = open_app_db(
        cx,
        move |db| {
            seed_week(db);
            db.conn()
                .execute(r#"UPDATE "Settings" SET reportCopyFormats = ?1, weeklyReportTemplate = ?2"#, [formats.as_str(), "## 主な作業\n（ここ）\n\n## 課題\n- {{期間}}"])
                .unwrap();
        },
        &base,
    );
    cx.simulate_keystrokes("cmd-2");
    wait_io(cx);
    (app, cx, log)
}

fn output(app: &gpui::Entity<crate::app::TrackApp>, cx: &mut gpui::VisualTestContext) -> gpui::Entity<crate::reports::CustomOutputView> {
    app.read_with(cx, |a, cx| a.reports.read(cx).output.clone().expect("output dialog"))
}

#[gpui::test]
fn custom_output_without_ai_uses_report_rows_and_copies_tsv(cx: &mut TestAppContext) {
    let (app, cx, _log) = ai_setup(cx, "");
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
    let (app, cx, log) = ai_setup(cx, r#"{"groups":[{"label":"定例","summary":"会議","entryIds":["e1","e2"]}]}"#);
    click(cx, "output-open");
    wait_io(cx);
    click(cx, "format-select");
    click(cx, "format-ai");
    click(cx, "output-create");
    wait_io(cx);
    let out = output(&app, cx);
    let rows = out.read_with(cx, |o, _| o.result.clone());
    assert_eq!((rows[0].label.as_str(), rows[0].minutes, rows[0].summary.as_str()), ("定例", 90, "会議"));
    let req = log.lock().unwrap().iter().find(|r| r.path == "/api/ai/generate-stream").cloned().expect("ai request");
    let body: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(body["mode"], "report-aggregation");
    assert!(body["input"].as_str().unwrap().contains("目的別"));
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
    let (app, cx, _log) = ai_setup(cx, "## 主な作業\n- AIの文章\n\n## 課題\n- なし");
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
    let (app, cx, _log) = ai_setup(cx, "x");
    app.update(cx, |a, cx| a.state.update(cx, |s, _| s.data.as_mut().unwrap().settings.weekly_report_template = "  ".into()));
    click(cx, "weekly-open");
    wait_io(cx);
    let weekly = app.read_with(cx, |a, cx| a.reports.read(cx).weekly.clone().unwrap());
    assert_eq!(weekly.read_with(cx, |w, _| w.error.clone()), Some("設定の「レポート」で出力テンプレートを保存してください。".into()));
}
