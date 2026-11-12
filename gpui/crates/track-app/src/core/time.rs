use chrono::{Datelike, Duration, NaiveDate};

pub use track_core::dates::{month_range, week_start};
#[cfg(test)]
pub use track_core::dates::week_range;

pub const SNAP_MIN: i64 = 15;
pub const DAY_MINUTES: i64 = 24 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DayCount {
    One,
    Three,
    /// 「稼働日」: the Sunday-start week filtered by work days.
    Workdays,
    Seven,
}

impl DayCount {
    pub fn label(self) -> &'static str {
        match self {
            Self::One => "1日",
            Self::Three => "3日",
            Self::Workdays => "稼働日",
            Self::Seven => "1週間",
        }
    }
}


pub fn visible_days(anchor: NaiveDate, count: DayCount, work_days: &[u32]) -> Vec<NaiveDate> {
    match count {
        DayCount::One => vec![anchor],
        DayCount::Three => (0..3).map(|i| anchor + Duration::days(i)).collect(),
        DayCount::Seven => (0..7).map(|i| week_start(anchor) + Duration::days(i)).collect(),
        DayCount::Workdays => (0..7)
            .map(|i| week_start(anchor) + Duration::days(i))
            .filter(|d| work_days.contains(&d.weekday().num_days_from_sunday()))
            .collect(),
    }
}

pub fn step_days(count: DayCount) -> i64 {
    match count {
        DayCount::One => 1,
        DayCount::Three => 3,
        DayCount::Workdays | DayCount::Seven => 7,
    }
}

/// Mirrors the React breakpoints: ≤639px always one day, ≤767px turns week views into three days.
pub fn effective_day_count(requested: DayCount, window_width: f32) -> DayCount {
    if window_width <= 639. {
        DayCount::One
    } else if window_width <= 767. && matches!(requested, DayCount::Workdays | DayCount::Seven) {
        DayCount::Three
    } else {
        requested
    }
}

/// Label for a range with exclusive `to`, as in `DateRangeNavigator.tsx`.
pub fn range_label(from: NaiveDate, to_exclusive: NaiveDate, is_month: bool) -> String {
    if is_month {
        return format!("{}月", from.month());
    }
    let last = to_exclusive - Duration::days(1);
    if last <= from {
        format!("{}月{}日", from.month(), from.day())
    } else if last.month() == from.month() && last.year() == from.year() {
        format!("{}月{}日 – {}日", from.month(), from.day(), last.day())
    } else {
        format!("{}月{}日 – {}月{}日", from.month(), from.day(), last.month(), last.day())
    }
}

/// `ReportsPage.tsx` formatDuration: "Xh Ym", or "Xh" when minutes are 0.
pub fn format_duration_report(minutes: i64) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    if m > 0 { format!("{h}h {m}m") } else { format!("{h}h") }
}

/// `EntryBlock.tsx` label: "Xh", "Xh Ym", or "Ym" under an hour.
pub fn format_duration_calendar(minutes: i64) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    if h > 0 {
        if m > 0 { format!("{h}h {m}m") } else { format!("{h}h") }
    } else {
        format!("{m}m")
    }
}

pub fn weekday_ja(d: NaiveDate) -> &'static str {
    ["日", "月", "火", "水", "木", "金", "土"][d.weekday().num_days_from_sunday() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn week_range_starts_sunday() {
        assert_eq!(week_range(d(2026, 11, 12)), (d(2026, 11, 8), d(2026, 11, 15)));
        assert_eq!(week_range(d(2026, 11, 8)), (d(2026, 11, 8), d(2026, 11, 15)));
    }

    #[test]
    fn month_range_ends_on_next_first() {
        assert_eq!(month_range(d(2026, 2, 10)), (d(2026, 2, 1), d(2026, 3, 1)));
        assert_eq!(month_range(d(2026, 12, 31)), (d(2026, 12, 1), d(2027, 1, 1)));
    }

    #[test]
    fn workdays_view_filters_by_work_days() {
        let days = visible_days(d(2026, 11, 12), DayCount::Workdays, &[1, 2, 3, 4, 5]);
        assert_eq!(days, (9..=13).map(|x| d(2026, 11, x)).collect::<Vec<_>>());
        assert_eq!(visible_days(d(2026, 11, 12), DayCount::Three, &[]), vec![d(2026, 11, 12), d(2026, 11, 13), d(2026, 11, 14)]);
        assert_eq!(visible_days(d(2026, 11, 12), DayCount::Seven, &[]).len(), 7);
        assert_eq!(step_days(DayCount::Workdays), 7);
        assert_eq!(step_days(DayCount::Three), 3);
    }

    #[test]
    fn effective_day_count_breakpoints() {
        assert_eq!(effective_day_count(DayCount::Seven, 639.), DayCount::One);
        assert_eq!(effective_day_count(DayCount::Seven, 700.), DayCount::Three);
        assert_eq!(effective_day_count(DayCount::Workdays, 700.), DayCount::Three);
        assert_eq!(effective_day_count(DayCount::One, 700.), DayCount::One);
        assert_eq!(effective_day_count(DayCount::Seven, 768.), DayCount::Seven);
    }

    #[test]
    fn range_labels() {
        assert_eq!(range_label(d(2026, 11, 8), d(2026, 11, 15), false), "11月8日 – 14日");
        assert_eq!(range_label(d(2026, 10, 26), d(2026, 11, 2), false), "10月26日 – 11月1日");
        assert_eq!(range_label(d(2026, 11, 12), d(2026, 11, 13), false), "11月12日");
        assert_eq!(range_label(d(2026, 11, 1), d(2026, 12, 1), true), "11月");
    }

    #[test]
    fn duration_formats() {
        assert_eq!(format_duration_report(30), "0h 30m");
        assert_eq!(format_duration_report(120), "2h");
        assert_eq!(format_duration_calendar(30), "30m");
        assert_eq!(format_duration_calendar(90), "1h 30m");
        assert_eq!(format_duration_calendar(60), "1h");
    }

    #[test]
    fn weekday_ja_sunday_first() {
        assert_eq!(weekday_ja(d(2026, 11, 8)), "日");
        assert_eq!(weekday_ja(d(2026, 11, 14)), "土");
    }
}
