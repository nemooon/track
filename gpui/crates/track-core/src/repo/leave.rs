//! 休暇: one `Leave` row per local date, plus the automatic full-day leave of a holiday on a work
//! day. A day's leave is counted from the current work schedule, less a fixed one-hour break.
use crate::error::Result;
use crate::holidays::holiday_name;
use crate::model::{Leave, LeaveKind, Settings};
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
        self.write_leave_row(date, stored)
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
        let settings = self.settings()?;
        let full = scheduled_minutes(&settings);
        let rows = self.leaves(from, to)?;
        Ok(from
            .iter_days()
            .take_while(|d| *d < to)
            .map(|d| {
                let minutes = match rows.iter().find(|l| l.date == d).map(|l| l.kind) {
                    Some(LeaveKind::Full) => full,
                    Some(LeaveKind::Am | LeaveKind::Pm) => full / 2,
                    Some(LeaveKind::None) => 0,
                    None if is_auto_leave(&settings, d) => full,
                    None => 0,
                };
                (d, minutes)
            })
            .collect())
    }
}
