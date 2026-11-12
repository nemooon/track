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
