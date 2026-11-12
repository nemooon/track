//! DB date-time format shared with Prisma: TEXT `YYYY-MM-DDTHH:mm:ss.SSS+00:00` (UTC).
use chrono::{DateTime, Utc};

pub fn to_db(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%S%.3f+00:00").to_string()
}

pub fn from_db(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|t| t.with_timezone(&Utc)).or_else(|| {
        // Prisma/SQLite may also hold "YYYY-MM-DD HH:MM:SS" (CURRENT_TIMESTAMP)
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok().map(|n| n.and_utc())
    })
}

pub fn now_db() -> String {
    to_db(Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_format_roundtrip() {
        let t: DateTime<Utc> = "2026-11-12T01:00:00Z".parse().unwrap();
        assert_eq!(to_db(t), "2026-11-12T01:00:00.000+00:00");
        assert_eq!(from_db("2026-11-12T01:00:00.000+00:00"), Some(t));
        assert_eq!(from_db("2026-11-12T01:00:00.000Z"), Some(t));
        assert_eq!(from_db("2026-11-12 01:00:00"), Some(t));
    }
}
