use crate::model::*;
use crate::Db;
use chrono::{DateTime, Local, TimeZone, Utc};

pub fn db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path()).unwrap();
    (dir, db)
}

pub fn local(d: u32, h: u32, m: u32) -> DateTime<Utc> {
    Local.with_ymd_and_hms(2026, 11, d, h, m, 0).unwrap().to_utc()
}

/// Client "Acme" with project "Alpha" (tag t-red attached to the project) and tag "t-blue".
pub fn seed(db: &mut Db) -> (String, String, String) {
    let c = db.create_client("Acme").unwrap();
    let p = db.create_project(&c, "Alpha", "#3b82f6").unwrap();
    let red = db.create_tag("red", "#ef4444").unwrap();
    let blue = db.create_tag("blue", "#3b82f6").unwrap();
    db.conn().execute(r#"INSERT INTO "TagOnProject"(id, tagId, projectId) VALUES ('tp1', ?1, ?2)"#, [&red, &p]).unwrap();
    (p, red, blue)
}

fn new_entry(start: DateTime<Utc>, end: DateTime<Utc>) -> NewEntry {
    NewEntry { start, end, title: Some("作業".into()), ..Default::default() }
}

#[test]
fn writes_rows_in_prisma_format() {
    let (_d, mut db) = db();
    let e = db.create_entry(new_entry("2026-11-12T01:00:00Z".parse().unwrap(), "2026-11-12T02:00:00Z".parse().unwrap())).unwrap();
    let (id, start, created, updated): (String, String, Option<String>, Option<String>) = db
        .conn()
        .query_row(r#"SELECT id, start, createdAt, updatedAt FROM "TimeEntry""#, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap();
    assert_eq!(id, e.id);
    assert_eq!(id.len(), 25);
    assert!(id.starts_with('c'));
    assert_eq!(start, "2026-11-12T01:00:00.000+00:00");
    assert!(created.is_some() && updated.is_some());
}

#[test]
fn entries_overlap_query_and_order() {
    let (_d, mut db) = db();
    let (p, red, blue) = seed(&mut db);
    let mut a = new_entry(local(12, 11, 0), local(12, 12, 0));
    a.project_id = Some(p.clone());
    a.tag_ids = vec![blue.clone()];
    db.create_entry(a).unwrap();
    db.create_entry(new_entry(local(12, 9, 0), local(12, 10, 0))).unwrap();
    db.create_entry(new_entry(local(13, 9, 0), local(13, 10, 0))).unwrap();
    let got = db.entries(local(12, 0, 0), local(13, 0, 0)).unwrap();
    assert_eq!(got.len(), 2);
    assert!(got[0].start < got[1].start);
    let e = &got[1];
    let proj = e.project.as_ref().unwrap();
    assert_eq!((proj.name.as_str(), proj.client.name.as_str()), ("Alpha", "Acme"));
    assert_eq!(proj.tags[0].tag.id, red);
    assert_eq!(e.tags[0].tag.id, blue);
    // touching boundaries do not overlap
    assert!(db.entries(local(12, 12, 0), local(12, 13, 0)).unwrap().is_empty());
}

#[test]
fn create_rounds_to_quarter() {
    let (_d, mut db) = db();
    let e = db.create_entry(new_entry(local(12, 10, 7), local(12, 10, 53))).unwrap();
    assert_eq!((e.start, e.end), (local(12, 10, 0), local(12, 11, 0)));
}

#[test]
fn create_rejects_cross_day_but_allows_midnight_end() {
    let (_d, mut db) = db();
    assert_eq!(db.create_entry(new_entry(local(12, 23, 0), local(13, 1, 0))).unwrap_err().code(), "entry_must_be_same_day");
    assert!(db.create_entry(new_entry(local(12, 23, 0), local(13, 0, 0))).is_ok());
    assert_eq!(db.create_entry(new_entry(local(12, 10, 0), local(12, 10, 5))).unwrap_err().code(), "invalid_rounded_interval");
    assert_eq!(db.create_entry(new_entry(local(12, 10, 0), local(12, 9, 0))).unwrap_err().code(), "invalid_input");
    let mut long = new_entry(local(12, 10, 0), local(12, 11, 0));
    long.title = Some("あ".repeat(101));
    assert_eq!(db.create_entry(long).unwrap_err().code(), "invalid_input");
    let mut brk = new_entry(local(12, 10, 0), local(12, 11, 0));
    brk.break_minutes = 601;
    assert_eq!(db.create_entry(brk).unwrap_err().code(), "invalid_input");
}

#[test]
fn update_replaces_tags_and_checks_same_day() {
    let (_d, mut db) = db();
    let (_p, red, blue) = seed(&mut db);
    let mut a = new_entry(local(12, 10, 0), local(12, 11, 0));
    a.tag_ids = vec![red.clone()];
    let e = db.create_entry(a).unwrap();
    let u = db
        .update_entry(&e.id, EntryPatch { tag_ids: Some(vec![blue.clone()]), title: Some(None), end: Some(local(12, 11, 37)), ..Default::default() })
        .unwrap();
    assert_eq!(u.tags.iter().map(|t| t.tag.id.clone()).collect::<Vec<_>>(), vec![blue]);
    assert_eq!(u.title, None);
    assert_eq!(u.end, local(12, 11, 30));
    assert_eq!(db.update_entry(&e.id, EntryPatch { end: Some(local(13, 2, 0)), ..Default::default() }).unwrap_err().code(), "entry_must_be_same_day");
    assert_eq!(db.update_entry("nope", EntryPatch::default()).unwrap_err().code(), "not_found");
}

#[test]
fn delete_removes_tag_links() {
    let (_d, mut db) = db();
    let (_p, red, _b) = seed(&mut db);
    let mut a = new_entry(local(12, 10, 0), local(12, 11, 0));
    a.tag_ids = vec![red];
    let e = db.create_entry(a).unwrap();
    db.delete_entry(&e.id).unwrap();
    let links: i64 = db.conn().query_row(r#"SELECT COUNT(*) FROM "TagOnEntry""#, [], |r| r.get(0)).unwrap();
    assert_eq!(links, 0);
    assert_eq!(db.delete_entry(&e.id).unwrap_err().code(), "not_found");
}

#[test]
fn reference_data_reads() {
    let (_d, mut db) = db();
    let (p, _red, _blue) = seed(&mut db);
    db.conn().execute(r#"UPDATE "Project" SET archived = 1 WHERE id = ?1"#, [&p]).unwrap();
    assert!(db.projects(false).unwrap().is_empty());
    assert_eq!(db.projects(true).unwrap().len(), 1);
    assert_eq!(db.clients().unwrap()[0].name, "Acme");
    assert_eq!(db.tags().unwrap().iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["blue", "red"]);
}

#[test]
fn settings_falls_back_to_standard_format() {
    let (_d, db) = db();
    let s = db.settings().unwrap();
    assert_eq!((s.work_start, s.work_end, s.work_days.clone()), (600, 1110, vec![1, 2, 3, 4, 5]));
    assert_eq!(s.report_copy_formats.len(), 1);
    assert_eq!(s.report_copy_formats[0].name, "標準");
    assert!(s.weekly_report_template.contains("## 今週の概要"));
}

#[test]
fn titles_newest_first_distinct_max_100() {
    let (_d, mut db) = db();
    for (i, t) in ["a", " b ", "a", "c"].iter().enumerate() {
        let mut e = new_entry(local(10 + i as u32, 9, 0), local(10 + i as u32, 10, 0));
        e.title = Some(t.to_string());
        db.create_entry(e).unwrap();
    }
    assert_eq!(db.entry_titles(Some("none")).unwrap(), vec!["c", "a", "b"]);
    assert!(db.entry_titles(Some("missing-project")).unwrap().is_empty());
}

#[test]
fn read_only_db_refuses_writes() {
    let (_d, mut db) = db();
    db.read_only = true;
    assert_eq!(db.create_entry(new_entry(local(12, 10, 0), local(12, 11, 0))).unwrap_err().code(), "schema_too_new");
}
