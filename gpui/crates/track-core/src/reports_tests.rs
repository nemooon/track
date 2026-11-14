use crate::model::*;
use crate::repo::repo_tests::{db, local, seed};
use crate::Db;
use chrono::{Local, NaiveDate, TimeZone};

fn q(range: ReportRange, group_by: GroupBy, day: u32) -> ReportQuery {
    ReportQuery {
        range,
        anchor: Local.from_local_datetime(&NaiveDate::from_ymd_opt(2026, 11, day).unwrap().and_hms_opt(0, 0, 0).unwrap()).unwrap().to_utc(),
        group_by,
        client_ids: vec![],
        project_ids: vec![],
        tag_ids: vec![],
    }
}

fn add(db: &mut Db, d: (u32, u32, u32), e: (u32, u32, u32), project: Option<&str>, tags: &[&str], brk: i64) -> TimeEntry {
    db.create_entry(NewEntry {
        project_id: project.map(String::from),
        start: local(d.0, d.1, d.2),
        end: local(e.0, e.1, e.2),
        title: Some("x".into()),
        tag_ids: tags.iter().map(|t| t.to_string()).collect(),
        break_minutes: brk,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn week_range_sunday_start() {
    let (_d, mut db) = db();
    add(&mut db, (7, 10, 0), (7, 11, 0), None, &[], 0); // Saturday before
    add(&mut db, (8, 10, 0), (8, 11, 0), None, &[], 0); // Sunday
    add(&mut db, (14, 10, 0), (14, 12, 0), None, &[], 0); // Saturday
    add(&mut db, (15, 10, 0), (15, 11, 0), None, &[], 0); // next Sunday
    let r = db.report(&q(ReportRange::Week, GroupBy::Project, 12)).unwrap();
    assert_eq!(r.total_minutes, 180);
    assert_eq!(r.range.from, local(8, 0, 0));
    assert_eq!(r.range.to, local(15, 0, 0));
    assert_eq!((r.rows[0].key.as_str(), r.rows[0].label.as_str(), r.rows[0].color.as_deref()), ("__none__", "プロジェクトなし", Some("#9ca3af")));
}

#[test]
fn clamps_to_range_and_prorates_break() {
    let (_d, mut db) = db();
    // Month view of November: an entry fully inside with a 30-minute break
    add(&mut db, (12, 9, 0), (12, 12, 0), None, &[], 30);
    let r = db.report(&q(ReportRange::Month, GroupBy::Project, 12)).unwrap();
    assert_eq!(r.total_minutes, 150);
    let es = db.report_entries(&q(ReportRange::Month, GroupBy::Project, 12)).unwrap();
    assert_eq!(es.entries[0].minutes, 150);
    assert_eq!(es.total_minutes, 150);
}

#[test]
fn none_filters_and_client_project_filters() {
    let (_d, mut db) = db();
    let (p, _red, _blue) = seed(&mut db);
    add(&mut db, (12, 9, 0), (12, 10, 0), Some(&p), &[], 0);
    add(&mut db, (12, 11, 0), (12, 13, 0), None, &[], 0);
    let mut query = q(ReportRange::Week, GroupBy::Project, 12);
    query.project_ids = vec!["__none__".into()];
    assert_eq!(db.report(&query).unwrap().total_minutes, 120);
    query.project_ids = vec!["__none__".into(), p.clone()];
    assert_eq!(db.report(&query).unwrap().total_minutes, 180);
    let client = db.projects(false).unwrap()[0].client_id.clone();
    query.project_ids = vec![];
    query.client_ids = vec![client];
    let r = db.report(&query).unwrap();
    assert_eq!(r.total_minutes, 60);
    assert_eq!(r.rows[0].label, "Acme · Alpha");
}

#[test]
fn tag_filter_matches_entry_or_project_tag_and_none() {
    let (_d, mut db) = db();
    let (p, red, blue) = seed(&mut db); // project Alpha carries `red`
    add(&mut db, (12, 9, 0), (12, 10, 0), Some(&p), &[], 0); // via project tag red
    add(&mut db, (12, 10, 0), (12, 11, 0), None, &[&blue], 0); // entry tag blue
    add(&mut db, (12, 11, 0), (12, 13, 0), None, &[], 0); // no tags at all
    let mut query = q(ReportRange::Week, GroupBy::Tag, 12);
    query.tag_ids = vec![red.clone()];
    let r = db.report(&query).unwrap();
    assert_eq!(r.total_minutes, 60);
    assert_eq!(r.rows.iter().map(|x| x.key.clone()).collect::<Vec<_>>(), vec![red.clone()]);
    query.tag_ids = vec!["__none__".into()];
    assert_eq!(db.report(&query).unwrap().total_minutes, 120);
    // tag filter on the entries endpoint
    query.tag_ids = vec![blue.clone()];
    assert_eq!(db.report_entries(&query).unwrap().entries.len(), 1);
}

#[test]
fn group_by_tag_counts_entry_once_in_total() {
    let (_d, mut db) = db();
    let (p, red, blue) = seed(&mut db);
    add(&mut db, (12, 9, 0), (12, 10, 0), Some(&p), &[&blue], 0); // red (project) + blue (entry)
    add(&mut db, (12, 10, 0), (12, 10, 30), None, &[], 0);
    let r = db.report(&q(ReportRange::Week, GroupBy::Tag, 12)).unwrap();
    assert_eq!(r.total_minutes, 90);
    let get = |k: &str| r.rows.iter().find(|x| x.key == k).map(|x| x.total_minutes);
    assert_eq!((get(&red), get(&blue), get("__none__")), (Some(60), Some(60), Some(30)));
    assert_eq!(r.rows.iter().find(|x| x.key == "__none__").unwrap().label, "タグなし");
}

#[test]
fn rows_sorted_by_minutes_desc_and_client_grouping() {
    let (_d, mut db) = db();
    let (p, _r, _b) = seed(&mut db);
    add(&mut db, (12, 9, 0), (12, 10, 0), Some(&p), &[], 0);
    add(&mut db, (12, 10, 0), (12, 13, 0), None, &[], 0);
    let r = db.report(&q(ReportRange::Week, GroupBy::Client, 12)).unwrap();
    assert_eq!(r.rows.iter().map(|x| x.total_minutes).collect::<Vec<_>>(), vec![180, 60]);
    assert_eq!((r.rows[1].label.as_str(), r.rows[1].color.as_deref()), ("Acme", None));
}

/// Same answers as the Bun server on the copy DB (`npm run local` on 8787 with the copy, --ignored).
#[test]
#[ignore]
fn matches_bun_server_on_copy_db() {
    let db = Db::open(&dirs::home_dir().unwrap().join(".track-gpui")).unwrap();
    let today = Local::now().date_naive();
    for range in [ReportRange::Week, ReportRange::Month] {
        for group_by in [GroupBy::Client, GroupBy::Project, GroupBy::Tag] {
            let query = ReportQuery {
                range,
                anchor: Local.from_local_datetime(&today.and_hms_opt(0, 0, 0).unwrap()).unwrap().to_utc(),
                group_by,
                client_ids: vec![],
                project_ids: vec![],
                tag_ids: vec![],
            };
            let ours = db.report(&query).unwrap();
            let url = format!(
                "http://127.0.0.1:8787/api/reports?range={}&anchor={}&groupBy={}",
                if range == ReportRange::Week { "week" } else { "month" },
                query.anchor.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                match group_by { GroupBy::Client => "client", GroupBy::Project => "project", GroupBy::Tag => "tag" }
            );
            let out = std::process::Command::new("curl").args(["-s", &url]).output().unwrap();
            let theirs: ReportResponse = serde_json::from_slice(&out.stdout).unwrap();
            let norm = |r: &ReportResponse| {
                let mut v: Vec<(String, String, Option<String>, i64)> = r.rows.iter().map(|x| (x.key.clone(), x.label.clone(), x.color.clone(), x.total_minutes)).collect();
                v.sort();
                (v, r.total_minutes)
            };
            assert_eq!(norm(&ours), norm(&theirs), "{range:?} {group_by:?}");
        }
    }
}

#[test]
fn weekly_copy_uses_projects_regardless_of_group_by() {
    let (_d, mut db) = db();
    let (p, red, _blue) = seed(&mut db);
    add(&mut db, (12, 9, 0), (12, 10, 30), Some(&p), &[&red], 0);
    add(&mut db, (12, 11, 0), (12, 11, 45), None, &[], 0);
    for g in [GroupBy::Tag, GroupBy::Client, GroupBy::Project] {
        assert_eq!(db.weekly_copy(&q(ReportRange::Week, g, 12)).unwrap(), "Alpha\t1.5\nプロジェクトなし\t0.75", "{g:?}");
    }
}

#[test]
fn weekly_copy_follows_filters() {
    let (_d, mut db) = db();
    let (p, _red, blue) = seed(&mut db);
    add(&mut db, (12, 9, 0), (12, 10, 0), Some(&p), &[], 0);
    add(&mut db, (12, 11, 0), (12, 13, 0), None, &[&blue], 0);
    let mut query = q(ReportRange::Week, GroupBy::Tag, 12);
    query.tag_ids = vec![blue];
    assert_eq!(db.weekly_copy(&query).unwrap(), "プロジェクトなし\t2");
    let mut query = q(ReportRange::Week, GroupBy::Project, 12);
    query.project_ids = vec![p];
    assert_eq!(db.weekly_copy(&query).unwrap(), "Alpha\t1");
}

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn heatmap_range_is_five_weeks_centred_on_the_week() {
    // 11/11 is a Wednesday; its week starts on Sunday 11/8.
    assert_eq!(crate::reports::heatmap_range(&q(ReportRange::Week, GroupBy::Project, 11)), (day(2026, 10, 25), day(2026, 11, 29)));
}

#[test]
fn heatmap_range_covers_the_months_weeks() {
    // November 2026 starts on a Sunday and ends on a Monday: five weeks.
    assert_eq!(crate::reports::heatmap_range(&q(ReportRange::Month, GroupBy::Project, 11)), (day(2026, 11, 1), day(2026, 12, 6)));
    // August 2026 starts on a Saturday and ends on a Monday: six weeks.
    let aug = ReportQuery { anchor: Local.from_local_datetime(&day(2026, 8, 10).and_hms_opt(0, 0, 0).unwrap()).unwrap().to_utc(), ..q(ReportRange::Month, GroupBy::Project, 1) };
    assert_eq!(crate::reports::heatmap_range(&aug), (day(2026, 7, 26), day(2026, 9, 6)));
}

#[test]
fn heatmap_sums_each_day_with_the_filters() {
    let (_d, mut db) = db();
    let (p, _, _) = seed(&mut db);
    add(&mut db, (10, 9, 0), (10, 11, 0), Some(&p), &[], 0);
    add(&mut db, (10, 13, 0), (10, 14, 0), None, &[], 0); // filtered out
    add(&mut db, (20, 10, 0), (20, 10, 30), Some(&p), &[], 0);
    let query = ReportQuery { project_ids: vec![p.clone()], ..q(ReportRange::Week, GroupBy::Project, 11) };
    let days = db.heatmap(&query).unwrap();
    assert_eq!(days.len(), 35, "one cell per day of the five weeks");
    let get = |d: NaiveDate| days.iter().find(|(x, _)| *x == d).map(|(_, m)| *m);
    assert_eq!(get(day(2026, 11, 10)), Some(120));
    assert_eq!(get(day(2026, 11, 20)), Some(30), "days outside the report's week count too");
    assert_eq!(get(day(2026, 11, 11)), Some(0));
}
