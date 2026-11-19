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

/// The local time for `naive`. A time skipped by a clock change (a DST gap, midnight in some
/// countries) becomes the first time that exists after it; a repeated one takes the earlier.
pub fn local_at(naive: chrono::NaiveDateTime) -> chrono::DateTime<chrono::Local> {
    use chrono::TimeZone;
    first_valid(naive, |n| chrono::Local.from_local_datetime(&n))
}

/// Local midnight of `d`, as UTC.
pub fn local_midnight(d: NaiveDate) -> chrono::DateTime<chrono::Utc> {
    local_at(d.and_hms_opt(0, 0, 0).unwrap()).to_utc()
}

fn first_valid<T>(naive: chrono::NaiveDateTime, at: impl Fn(chrono::NaiveDateTime) -> chrono::LocalResult<T>) -> T {
    // Gaps are at most a few hours; step a quarter hour at a time.
    for step in 0..=96 {
        if let Some(t) = at(naive + Duration::minutes(15 * step)).earliest() {
            return t;
        }
    }
    panic!("no valid local time within a day of {naive}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{LocalResult, NaiveTime};

    #[test]
    fn a_skipped_local_time_moves_to_the_first_valid_one() {
        // Like Chile: the clocks jump from 00:00 to 01:00, so midnight does not exist.
        let day = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let resolve = |n: chrono::NaiveDateTime| if n.time() < NaiveTime::from_hms_opt(1, 0, 0).unwrap() { LocalResult::None } else { LocalResult::Single(n) };
        assert_eq!(first_valid(day.and_hms_opt(0, 0, 0).unwrap(), resolve), day.and_hms_opt(1, 0, 0).unwrap());
    }

    #[test]
    fn a_repeated_local_time_takes_the_earlier_one() {
        let n = NaiveDate::from_ymd_opt(2026, 4, 5).unwrap().and_hms_opt(0, 0, 0).unwrap();
        assert_eq!(first_valid(n, |n| LocalResult::Ambiguous(n, n + Duration::hours(1))), n);
    }
}
