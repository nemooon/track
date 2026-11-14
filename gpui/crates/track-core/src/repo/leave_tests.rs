use crate::model::*;
use crate::repo::repo_tests::db;
use crate::repo::scheduled_minutes;
use crate::Db;
use chrono::NaiveDate;

fn day(m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, m, d).unwrap()
}

/// 10:00–18:30, Monday to Friday.
fn schedule(db: &mut Db) {
    db.update_settings(SettingsPatch { work_start: Some(600), work_end: Some(1110), work_days: Some(vec![1, 2, 3, 4, 5]), ..Default::default() }).unwrap();
}

fn minutes(db: &Db, d: NaiveDate) -> i64 {
    db.leave_minutes(d, d.succ_opt().unwrap()).unwrap()[0].1
}

#[test]
fn scheduled_is_work_hours_less_an_hour() {
    let (_d, mut db) = db();
    schedule(&mut db);
    assert_eq!(scheduled_minutes(&db.settings().unwrap()), 450);
}

#[test]
fn full_and_half_days() {
    let (_d, mut db) = db();
    schedule(&mut db);
    let d = day(11, 12);
    db.set_leave(d, Some(LeaveKind::Full)).unwrap();
    assert_eq!(minutes(&db, d), 450);
    db.set_leave(d, Some(LeaveKind::Am)).unwrap();
    assert_eq!(minutes(&db, d), 225);
    db.set_leave(d, Some(LeaveKind::Pm)).unwrap();
    assert_eq!(minutes(&db, d), 225);
    assert_eq!(db.leaves(d, d.succ_opt().unwrap()).unwrap(), vec![Leave { date: d, kind: LeaveKind::Pm }]);
}

#[test]
fn workday_holidays_are_full_leave() {
    let (_d, mut db) = db();
    schedule(&mut db);
    assert_eq!(minutes(&db, day(11, 3)), 450, "文化の日 (Tuesday)");
    assert_eq!(minutes(&db, day(5, 3)), 0, "憲法記念日 2026 is a Sunday: not a work day");
}

#[test]
fn none_cancels_a_holiday() {
    let (_d, mut db) = db();
    schedule(&mut db);
    db.set_leave(day(11, 3), None).unwrap();
    assert_eq!(minutes(&db, day(11, 3)), 0);
    assert_eq!(db.leaves(day(11, 3), day(11, 4)).unwrap(), vec![Leave { date: day(11, 3), kind: LeaveKind::None }]);
    let plain = day(11, 12);
    db.set_leave(plain, Some(LeaveKind::Full)).unwrap();
    db.set_leave(plain, None).unwrap();
    assert!(db.leaves(plain, plain.succ_opt().unwrap()).unwrap().is_empty(), "a plain day's 「なし」 removes the row");
}

#[test]
fn manual_leave_counts_on_any_day() {
    let (_d, mut db) = db();
    schedule(&mut db);
    let sunday = day(11, 15);
    db.set_leave(sunday, Some(LeaveKind::Full)).unwrap();
    assert_eq!(minutes(&db, sunday), 450);
}

#[test]
fn leave_follows_the_current_schedule() {
    let (_d, mut db) = db();
    schedule(&mut db);
    let d = day(11, 12);
    db.set_leave(d, Some(LeaveKind::Full)).unwrap();
    db.update_settings(SettingsPatch { work_start: Some(540), work_end: Some(1080), ..Default::default() }).unwrap();
    assert_eq!(minutes(&db, d), 480);
}

#[test]
fn leave_minutes_lists_every_day() {
    let (_d, mut db) = db();
    schedule(&mut db);
    let days = db.leave_minutes(day(11, 1), day(11, 8)).unwrap();
    assert_eq!(days.len(), 7);
    assert_eq!(days.iter().map(|(_, m)| m).sum::<i64>(), 450, "only 11/3");
}

fn at(d: NaiveDate, minutes: u32) -> chrono::DateTime<chrono::Utc> {
    crate::dates::local_at(d.and_hms_opt(minutes / 60, minutes % 60, 0).unwrap()).to_utc()
}

#[test]
fn direct_is_one_day_of_work() {
    let (_d, mut db) = db();
    schedule(&mut db);
    let (p, _, _) = crate::repo::repo_tests::seed(&mut db);
    let d = day(11, 12);
    let e = db.set_direct(d, &p).unwrap();
    assert_eq!((e.start, e.end, e.break_minutes, e.kind), (at(d, 600), at(d, 1110), 60, EntryKind::Direct));
    assert_eq!((e.title.as_deref(), e.project_id.as_deref()), (Some("直行直帰"), Some(p.as_str())));
    assert_eq!((e.end - e.start).num_minutes() - e.break_minutes, 450);
    assert_eq!(db.direct_entry(d).unwrap().map(|x| x.id), Some(e.id));
}

#[test]
fn direct_replaces_the_days_direct() {
    let (_d, mut db) = db();
    schedule(&mut db);
    let (p, _, _) = crate::repo::repo_tests::seed(&mut db);
    let c = db.projects(true).unwrap()[0].client_id.clone();
    let q = db.create_project(&c, "Other", "#ef4444").unwrap();
    let d = day(11, 12);
    db.set_direct(d, &p).unwrap();
    db.set_direct(d, &q).unwrap();
    let all: Vec<_> = db.entries(at(d, 0), at(d.succ_opt().unwrap(), 0)).unwrap().into_iter().filter(|e| e.kind == EntryKind::Direct).collect();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].project_id.as_deref(), Some(q.as_str()));
}

#[test]
fn direct_clears_leave() {
    let (_d, mut db) = db();
    schedule(&mut db);
    let (p, _, _) = crate::repo::repo_tests::seed(&mut db);
    let d = day(11, 12);
    db.set_leave(d, Some(LeaveKind::Full)).unwrap();
    db.set_direct(d, &p).unwrap();
    assert!(db.leaves(d, d.succ_opt().unwrap()).unwrap().is_empty());
    assert_eq!(minutes(&db, d), 0);
}

#[test]
fn leave_clears_direct() {
    let (_d, mut db) = db();
    schedule(&mut db);
    let (p, _, _) = crate::repo::repo_tests::seed(&mut db);
    let d = day(11, 12);
    db.set_direct(d, &p).unwrap();
    db.set_leave(d, Some(LeaveKind::Am)).unwrap();
    assert!(db.direct_entry(d).unwrap().is_none());
    assert_eq!(minutes(&db, d), 225);
}

#[test]
fn direct_on_a_holiday_cancels_the_auto_leave() {
    let (_d, mut db) = db();
    schedule(&mut db);
    let (p, _, _) = crate::repo::repo_tests::seed(&mut db);
    db.set_direct(day(11, 3), &p).unwrap();
    assert_eq!(minutes(&db, day(11, 3)), 0);
}

#[test]
fn remove_direct_deletes_it() {
    let (_d, mut db) = db();
    schedule(&mut db);
    let (p, _, _) = crate::repo::repo_tests::seed(&mut db);
    let d = day(11, 12);
    db.set_direct(d, &p).unwrap();
    db.remove_direct(d).unwrap();
    assert!(db.direct_entry(d).unwrap().is_none());
}
