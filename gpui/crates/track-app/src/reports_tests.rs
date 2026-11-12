use crate::api::GroupBy;
use crate::reports::Children;
use crate::testing::{open_app_with, stub_server, Recorded};
use gpui::{Modifiers, TestAppContext};
use std::sync::{Arc, Mutex};

const REPORT: &str = r##"{"rows":[{"key":"p1","label":"Acme · Alpha","color":"#3b82f6","totalMinutes":90},{"key":"__none__","label":"プロジェクトなし","color":"#9ca3af","totalMinutes":30}],"totalMinutes":120,"range":{"from":"2026-11-07T15:00:00.000Z","to":"2026-11-14T15:00:00.000Z"}}"##;
const ENTRIES: &str = r##"{"entries":[
 {"id":"e1","start":"2026-11-10T01:00:00.000Z","end":"2026-11-10T01:30:00.000Z","minutes":30,"title":"定例","note":null,"project":{"id":"p1","name":"Alpha","color":"#3b82f6","client":{"id":"c1","name":"Acme"}},"tags":[]},
 {"id":"e2","start":"2026-11-11T01:00:00.000Z","end":"2026-11-11T02:00:00.000Z","minutes":60,"title":"定例","note":null,"project":{"id":"p1","name":"Alpha","color":"#3b82f6","client":{"id":"c1","name":"Acme"}},"tags":[]}
],"totalMinutes":90}"##;

fn wait_io(cx: &mut gpui::VisualTestContext) {
    for _ in 0..60 {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn setup(cx: &mut TestAppContext) -> (gpui::Entity<crate::app::TrackApp>, &mut gpui::VisualTestContext, Arc<Mutex<Vec<Recorded>>>) {
    let (base, log) = stub_server(|_, path| match path {
        "/api/reports" => (200, REPORT.into()),
        "/api/reports/entries" => (200, ENTRIES.into()),
        "/api/entries" => (200, "[]".into()),
        _ => (404, r#"{"error":"nf"}"#.into()),
    });
    let (app, cx) = open_app_with(cx, base);
    cx.simulate_keystrokes("cmd-2");
    wait_io(cx);
    (app, cx, log)
}

fn click(cx: &mut gpui::VisualTestContext, selector: &'static str) {
    let b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} not rendered"));
    cx.simulate_click(b.center(), Modifiers::none());
}

fn last_get(log: &Arc<Mutex<Vec<Recorded>>>, path: &str) -> String {
    log.lock().unwrap().iter().rev().find(|r| r.path.starts_with(path) && r.path[path.len()..].starts_with('?')).map(|r| r.path.clone()).expect("request")
}

#[gpui::test]
fn loads_report_and_expands_row_with_child_filters(cx: &mut TestAppContext) {
    let (app, cx, log) = setup(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).data.as_ref().map(|d| d.rows.len())), Some(2));
    let q = last_get(&log, "/api/reports");
    assert!(q.contains("range=week") && q.contains("groupBy=project"), "{q}");
    click(cx, "row-p1");
    wait_io(cx);
    let child = last_get(&log, "/api/reports/entries");
    assert!(child.contains("projectIds=p1") && !child.contains("groupBy"), "{child}");
    let loaded = app.read_with(cx, |a, cx| a.reports.read(cx).children.get("project:p1").cloned());
    assert!(matches!(loaded, Some(Children::Loaded(ref v)) if v.len() == 2));
    // expanded state is persisted per groupBy
    assert_eq!(app.read_with(cx, |a, cx| a.state.read(cx).persist.reports_expanded.get("project").cloned()), Some(vec!["project:p1".to_string()]));
}

#[gpui::test]
fn toggle_all_expands_every_row_then_collapses(cx: &mut TestAppContext) {
    let (app, cx, _log) = setup(cx);
    click(cx, "toggle-all");
    wait_io(cx);
    assert_eq!(app.read_with(cx, |a, cx| a.reports.read(cx).expanded.len()), 2);
    click(cx, "toggle-all");
    assert!(app.read_with(cx, |a, cx| a.reports.read(cx).expanded.is_empty()));
}

#[gpui::test]
fn project_filter_requery_and_client_change_drops_projects(cx: &mut TestAppContext) {
    let (app, cx, log) = setup(cx);
    click(cx, "filter-プロジェクト");
    click(cx, "filter-option-p2");
    wait_io(cx);
    assert!(last_get(&log, "/api/reports").contains("projectIds=p2"));
    // choosing a client that p2 does not belong to removes it
    app.update(cx, |a, cx| a.reports.update(cx, |r, cx| r.set_selected(crate::reports::FilterKind::Client, vec!["zz".into()], cx)));
    assert!(app.read_with(cx, |a, cx| a.reports.read(cx).project_ids.is_empty()));
}

#[gpui::test]
fn group_by_switch_and_same_titles_toggle(cx: &mut TestAppContext) {
    let (app, cx, log) = setup(cx);
    app.update(cx, |a, cx| a.reports.update(cx, |r, cx| r.set_group_by(GroupBy::Tag, cx)));
    wait_io(cx);
    assert!(last_get(&log, "/api/reports").contains("groupBy=tag"));
    click(cx, "group-same-titles");
    assert!(app.read_with(cx, |a, cx| a.state.read(cx).persist.group_same_titles));
}

fn ai_setup<'a>(cx: &'a mut TestAppContext, ai_text: &'static str) -> (gpui::Entity<crate::app::TrackApp>, &'a mut gpui::VisualTestContext, Arc<Mutex<Vec<Recorded>>>) {
    let (base, log) = stub_server(move |_, path| match path {
        "/api/reports" => (200, REPORT.into()),
        "/api/reports/entries" => (200, ENTRIES.into()),
        "/api/entries" => (200, "[]".into()),
        "/api/ai/generate-stream" => {
            let result = serde_json::json!({"type": "result", "text": ai_text, "provider": "codex"}).to_string();
            (200, format!("{{\"type\":\"progress\",\"kind\":\"reasoning\",\"message\":\"分類中\"}}\n{{\"type\":\"heartbeat\"}}\n{result}\n"))
        }
        _ => (404, r#"{"error":"nf"}"#.into()),
    });
    let (app, cx) = open_app_with(cx, base);
    app.update(cx, |a, cx| {
        a.state.update(cx, |s, _| {
            let d = s.data.as_mut().unwrap();
            let col = |id: &str, field: crate::api::ReportCopyField| crate::api::ReportCopyColumn::Field { id: id.into(), field, label: None, duration_format: None };
            d.settings.report_copy_formats = vec![
                crate::api::ReportCopyFormat { id: "plain".into(), name: "標準".into(), target: "ai-aggregation".into(), delimiter: "tab".into(), include_header: true, ai_prompt: "".into(), columns: vec![col("c", crate::api::ReportCopyField::Category), col("n", crate::api::ReportCopyField::EntryCount)] },
                crate::api::ReportCopyFormat { id: "ai".into(), name: "AI".into(), target: "ai-aggregation".into(), delimiter: "comma".into(), include_header: false, ai_prompt: "目的別".into(), columns: vec![col("c", crate::api::ReportCopyField::Category), col("s", crate::api::ReportCopyField::Summary)] },
            ];
            d.settings.weekly_report_template = "## 主な作業\n（ここ）\n\n## 課題\n- {{期間}}".into();
        })
    });
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
    assert_eq!(rows.iter().map(|r| (r.label.as_str(), r.entry_count)).collect::<Vec<_>>(), vec![("Alpha", 2), ("プロジェクトなし", 2)]);
    click(cx, "output-copy");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()), Some("集計名\t件数\nAlpha\t2\nプロジェクトなし\t2".into()));
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
    cx.update(|w, cx| gpui_component::WindowExt::close_dialog(w, cx));
    click(cx, "output-open");
    wait_io(cx);
    click(cx, "format-select");
    click(cx, "format-ai");
    wait_io(cx);
    let out = output(&app, cx);
    assert!(out.read_with(cx, |o, _| o.restored_at.is_some()));
    assert_eq!(out.read_with(cx, |o, _| o.result.len()), 1);
}

#[gpui::test]
fn weekly_report_replaces_main_work_with_fixed_text(cx: &mut TestAppContext) {
    let (app, cx, _log) = ai_setup(cx, "## 主な作業\n- AIの文章\n\n## 課題\n- なし");
    click(cx, "weekly-open");
    wait_io(cx);
    let weekly = app.read_with(cx, |a, cx| a.reports.read(cx).weekly.clone().unwrap());
    let text = weekly.read_with(cx, |w, cx| w.editor.read(cx).value().to_string());
    assert_eq!(text, "## 主な作業\n\n- **Acme / Alpha**（1時間30分）：定例\n\n## 課題\n- なし");
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
