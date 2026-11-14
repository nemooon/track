//! 休暇: one `Leave` row per local date, plus the automatic full-day leave of a holiday on a work
//! day. A day's leave is counted from the current work schedule, less a fixed one-hour break.
use crate::error::Result;
use crate::holidays::holiday_name;
use crate::model::{EntryKind, Leave, LeaveDay, LeaveDayKind, LeaveKind, NewEntry, Settings, TimeEntry};
use crate::{Db, ids, time};
use chrono::{Datelike, NaiveDate};
use rusqlite::params;

/// The break taken out of a working day (fixed; there is no setting for it).
const BREAK_MINUTES: i64 = 60;

/// One day of scheduled work: the work hours less the break.
pub fn scheduled_minutes(s: &Settings) -> i64 {
    (s.work_end - s.work_start - BREAK_MINUTES).max(0)
}

fn is_work_day(s: &Settings, d: NaiveDate) -> bool {
    s.work_days.contains(&d.weekday().num_days_from_sunday())
}

/// A holiday on a work day: full leave unless a row says otherwise.
fn is_auto_leave(s: &Settings, d: NaiveDate) -> bool {
    is_work_day(s, d) && holiday_name(d).is_some()
}

impl Db {
    /// The stored rows in `[from, to)`, by date.
    pub fn leaves(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<Leave>> {
        let mut st = self.conn().prepare(r#"SELECT date, kind FROM "Leave" WHERE date >= ?1 AND date < ?2 ORDER BY date"#)?;
        let rows = st.query_map(params![from.to_string(), to.to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (date, kind) = row?;
            if let (Ok(date), Some(kind)) = (date.parse::<NaiveDate>(), LeaveKind::from_db(&kind)) {
                out.push(Leave { date, kind });
            }
        }
        Ok(out)
    }

    /// Sets a day's leave. `None` is 「なし」: on a holiday that is a work day it stores `none`
    /// (cancelling the automatic leave); on any other day it removes the row.
    pub fn set_leave(&mut self, date: NaiveDate, kind: Option<LeaveKind>) -> Result<()> {
        self.ensure_writable()?;
        let settings = self.settings()?;
        let stored = match kind {
            Some(k) => Some(k),
            None if is_auto_leave(&settings, date) => Some(LeaveKind::None),
            None => None,
        };
        self.in_savepoint(|db| {
            // Leave and a direct visit never share a day: the later choice wins.
            if kind.is_some() {
                db.remove_direct_inner(date)?;
            }
            db.write_leave_row(date, stored)
        })
    }

    /// Runs `f` inside a named savepoint: all of it or none of it.
    fn in_savepoint<T>(&mut self, f: impl FnOnce(&mut Db) -> Result<T>) -> Result<T> {
        self.conn().execute_batch("SAVEPOINT leave_direct")?;
        let undo = |db: &Db| {
            let _ = db.conn().execute_batch("ROLLBACK TO leave_direct; RELEASE leave_direct");
        };
        match f(self) {
            // If even the release fails, roll back so no transaction is left open.
            Ok(v) => match self.conn().execute_batch("RELEASE leave_direct") {
                Ok(()) => Ok(v),
                Err(e) => {
                    undo(self);
                    Err(e.into())
                }
            },
            Err(e) => {
                undo(self);
                Err(e)
            }
        }
    }

    /// The day's 直行直帰 entry, if any.
    pub fn direct_entry(&self, date: NaiveDate) -> Result<Option<TimeEntry>> {
        let (from, to) = (crate::dates::local_midnight(date), crate::dates::local_midnight(date + chrono::Duration::days(1)));
        Ok(self.entries(from, to)?.into_iter().find(|e| e.kind == EntryKind::Direct && e.start >= from && e.start < to))
    }

    /// Makes (or replaces) the day's 直行直帰: one day of scheduled work for `project_id`, from the
    /// work start to the work end with the one-hour break. The day's leave goes (a holiday's
    /// automatic leave is cancelled with `none`).
    pub fn set_direct(&mut self, date: NaiveDate, project_id: &str) -> Result<TimeEntry> {
        self.ensure_writable()?;
        let settings = self.settings()?;
        let at = |minutes: i64| crate::dates::local_at(date.and_hms_opt(0, 0, 0).unwrap() + chrono::Duration::minutes(minutes)).to_utc();
        self.in_savepoint(|db| {
            db.remove_direct_inner(date)?;
            let e = db.create_entry(NewEntry {
                project_id: Some(project_id.to_string()),
                start: at(settings.work_start),
                end: at(settings.work_end),
                title: Some("直行直帰".into()),
                break_minutes: BREAK_MINUTES,
                ..Default::default()
            })?;
            db.conn().execute(r#"UPDATE "TimeEntry" SET kind = ?1 WHERE id = ?2"#, params![EntryKind::Direct.as_db(), e.id])?;
            db.write_leave_row(date, is_auto_leave(&settings, date).then_some(LeaveKind::None))?;
            Ok(TimeEntry { kind: EntryKind::Direct, ..e })
        })
    }

    /// Removes the day's 直行直帰, if any. On a holiday the `none` the direct visit left goes too,
    /// so the day is the holiday's leave again.
    pub fn remove_direct(&mut self, date: NaiveDate) -> Result<()> {
        self.ensure_writable()?;
        let settings = self.settings()?;
        self.in_savepoint(|db| {
            let had = db.direct_entry(date)?.is_some();
            db.remove_direct_inner(date)?;
            if had && is_auto_leave(&settings, date) && db.leaves(date, date + chrono::Duration::days(1))?.first().map(|l| l.kind) == Some(LeaveKind::None) {
                db.write_leave_row(date, None)?;
            }
            Ok(())
        })
    }

    fn remove_direct_inner(&mut self, date: NaiveDate) -> Result<()> {
        if let Some(e) = self.direct_entry(date)? {
            self.delete_entry(&e.id)?;
        }
        Ok(())
    }

    /// Upserts (or, for `None`, deletes) the day's row.
    pub(crate) fn write_leave_row(&self, date: NaiveDate, kind: Option<LeaveKind>) -> Result<()> {
        match kind {
            Some(k) => {
                let now = time::now_db();
                self.conn().execute(
                    r#"INSERT INTO "Leave"(id, date, kind, createdAt, updatedAt) VALUES (?1, ?2, ?3, ?4, ?4)
                       ON CONFLICT(date) DO UPDATE SET kind = excluded.kind, updatedAt = excluded.updatedAt"#,
                    params![ids::cuid(), date.to_string(), k.as_db(), now],
                )?;
            }
            None => {
                self.conn().execute(r#"DELETE FROM "Leave" WHERE date = ?1"#, [date.to_string()])?;
            }
        }
        Ok(())
    }

    /// Leave minutes for every day in `[from, to)`, in order (0 for days without leave).
    pub fn leave_minutes(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, i64)>> {
        let days = self.leave_days(from, to)?;
        Ok(from.iter_days().take_while(|d| *d < to).map(|d| (d, days.iter().find(|l| l.date == d).map_or(0, |l| l.minutes))).collect())
    }

    /// The days in `[from, to)` that count as leave, in order, each with its kind and minutes.
    pub fn leave_days(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<LeaveDay>> {
        let settings = self.settings()?;
        let full = scheduled_minutes(&settings);
        let rows = self.leaves(from, to)?;
        Ok(from
            .iter_days()
            .take_while(|d| *d < to)
            .filter_map(|d| {
                let kind = match rows.iter().find(|l| l.date == d).map(|l| l.kind) {
                    Some(LeaveKind::Full) => LeaveDayKind::Full,
                    Some(LeaveKind::Am) => LeaveDayKind::Am,
                    Some(LeaveKind::Pm) => LeaveDayKind::Pm,
                    Some(LeaveKind::None) => return None,
                    None if is_auto_leave(&settings, d) => LeaveDayKind::Holiday(holiday_name(d)?.to_string()),
                    None => return None,
                };
                let minutes = if matches!(kind, LeaveDayKind::Am | LeaveDayKind::Pm) { full / 2 } else { full };
                (minutes > 0).then_some(LeaveDay { date: d, kind, minutes })
            })
            .collect())
    }
}
