//! Calendar ranges shared by reports and the app (Sunday-start weeks, local dates).
use chrono::{Datelike, Duration, NaiveDate};

pub fn week_start(anchor: NaiveDate) -> NaiveDate {
    anchor - Duration::days(anchor.weekday().num_days_from_sunday() as i64)
}

/// Sunday-start week; `to` is exclusive.
pub fn week_range(anchor: NaiveDate) -> (NaiveDate, NaiveDate) {
    let from = week_start(anchor);
    (from, from + Duration::days(7))
}

/// Calendar month; `to` is the first day of the next month (exclusive).
pub fn month_range(anchor: NaiveDate) -> (NaiveDate, NaiveDate) {
    let from = anchor.with_day(1).unwrap();
    let to = if from.month() == 12 {
        NaiveDate::from_ymd_opt(from.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(from.year(), from.month() + 1, 1).unwrap()
    };
    (from, to)
}
