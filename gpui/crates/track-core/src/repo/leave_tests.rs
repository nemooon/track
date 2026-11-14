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
