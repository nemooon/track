//! Entry rules shared with `src/shared/validators.ts` and `src/server/routes/entries.ts`.
use crate::error::{CoreError, Result};
use chrono::{DateTime, Local, Timelike, Utc};

pub const SNAP_MS: i64 = 15 * 60 * 1000;

/// `snapToQuarter`: nearest 15 minutes.
pub fn snap_to_quarter(t: DateTime<Utc>) -> DateTime<Utc> {
    let ms = t.timestamp_millis();
    let snapped = ((ms as f64) / SNAP_MS as f64).round() as i64 * SNAP_MS;
    DateTime::from_timestamp_millis(snapped).unwrap()
}

pub fn is_quarter(t: DateTime<Utc>) -> bool {
    t.timestamp_millis() % SNAP_MS == 0
}

/// Same local calendar day, or the end is exactly local 00:00 (belongs to the previous day).
pub fn check_same_day(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<()> {
    let (s, e) = (start.with_timezone(&Local), end.with_timezone(&Local));
    if s.date_naive() == e.date_naive() || (e.hour() == 0 && e.minute() == 0) {
        Ok(())
    } else {
        Err(CoreError::code_msg("entry_must_be_same_day", "開始と終了は同じ日にしてください。"))
    }
}

pub fn invalid(msg: impl Into<String>) -> CoreError {
    CoreError::code_msg("invalid_input", msg)
}

pub fn check_text(title: &Option<String>, note: &Option<String>) -> Result<()> {
    if title.as_ref().is_some_and(|t| t.chars().count() > 100) {
        return Err(invalid("タイトルは100文字以内にしてください。"));
    }
    if note.as_ref().is_some_and(|t| t.chars().count() > 500) {
        return Err(invalid("メモは500文字以内にしてください。"));
    }
    Ok(())
}

pub fn check_break(minutes: i64) -> Result<()> {
    if (0..=600).contains(&minutes) { Ok(()) } else { Err(invalid("休憩は0〜600分にしてください。")) }
}

/// Dedupe and sort (create path).
pub fn normalize_tag_ids(ids: &[String]) -> Vec<String> {
    let mut v: Vec<String> = ids.iter().filter(|s| !s.is_empty()).cloned().collect();
    v.sort();
    v.dedup();
    v
}
