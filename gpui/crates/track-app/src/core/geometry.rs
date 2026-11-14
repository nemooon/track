use super::time::{DAY_MINUTES, SNAP_MIN};
use chrono::{DateTime, Local, NaiveDate};

/// Hatched buffer above 0:00 and below 24:00.
pub const BUFFER_PX: f32 = 32.;
pub const GUTTER_PX: f32 = 60.;
/// Pixels per hour. The middle (default) level gives a 15-minute entry room for one line.
pub const ZOOM_LEVELS: [f32; 3] = [48., 120., 192.];
pub const DEFAULT_ZOOM: usize = 1;

pub fn snap_px(hour_px: f32) -> f32 {
    hour_px * SNAP_MIN as f32 / 60.
}

/// Y (from 0:00) to minutes, rounded to the 15-minute snap and clamped to 0..=1440.
pub fn y_to_minutes(y: f32, hour_px: f32) -> i64 {
    let snapped = (y / snap_px(hour_px)).round() as i64 * SNAP_MIN;
    snapped.clamp(0, DAY_MINUTES)
}

pub fn minutes_to_y(minutes: i64, hour_px: f32) -> f32 {
    minutes as f32 / 60. * hour_px
}

/// Minutes from `day` 00:00 to `t`; the next day's 00:00 yields 1440.
pub fn minutes_of_day(t: DateTime<Local>, day: NaiveDate) -> i64 {
    (t - at_minutes(day, 0)).num_minutes()
}

pub fn at_minutes(day: NaiveDate, minutes: i64) -> DateTime<Local> {
    let midnight = track_core::dates::local_at(day.and_hms_opt(0, 0, 0).unwrap());
    midnight + chrono::Duration::minutes(minutes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone};

    #[test]
    fn y_to_minutes_snaps_and_clamps() {
        assert_eq!(y_to_minutes(50., 96.), 30);
        assert_eq!(y_to_minutes(-10., 96.), 0);
        assert_eq!(y_to_minutes(99999., 96.), 1440);
        assert_eq!(minutes_to_y(90, 48.), 72.);
    }

    #[test]
    fn at_minutes_and_back() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let t = at_minutes(day, 615);
        assert_eq!(t, Local.with_ymd_and_hms(2026, 11, 12, 10, 15, 0).unwrap());
        assert_eq!(minutes_of_day(t, day), 615);
        assert_eq!(minutes_of_day(at_minutes(day, 1440), day), 1440);
    }
}
