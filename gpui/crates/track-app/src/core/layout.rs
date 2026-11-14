use super::geometry::minutes_of_day;
use track_core::model::TimeEntry;
use chrono::{Local, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Laid {
    pub index: usize,
    pub lane: usize,
    pub lane_count: usize,
    /// Lanes the block covers from `lane` rightwards: the ones beside it that are free for its
    /// whole span (at least 1).
    pub span: usize,
}

/// Sweep-line lane assignment (same algorithm as `calendar/layout.ts`).
/// `span` returns (start, end) in any monotonic unit.
pub fn layout_blocks<T>(items: &[T], span: impl Fn(&T) -> (i64, i64)) -> Vec<Laid> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| {
        let (sa, ea) = span(&items[a]);
        let (sb, eb) = span(&items[b]);
        sa.cmp(&sb).then(eb.cmp(&ea))
    });
    let mut out = Vec::with_capacity(items.len());
    let mut group: Vec<(usize, usize, i64, i64)> = Vec::new(); // (index, lane, start, end)
    let mut group_end = i64::MIN;
    let flush = |group: &mut Vec<(usize, usize, i64, i64)>, out: &mut Vec<Laid>| {
        let count = group.iter().map(|g| g.1 + 1).max().unwrap_or(0);
        let busy = |lane: usize, s: i64, e: i64| group.iter().any(|g| g.1 == lane && g.2 < e && s < g.3);
        let laid: Vec<Laid> = group
            .iter()
            .map(|&(index, lane, s, e)| {
                let span = 1 + (lane + 1..count).take_while(|&l| !busy(l, s, e)).count();
                Laid { index, lane, lane_count: count, span }
            })
            .collect();
        group.clear();
        out.extend(laid);
    };
    for i in order {
        let (s, e) = span(&items[i]);
        if !group.is_empty() && s >= group_end {
            flush(&mut group, &mut out);
            group_end = i64::MIN;
        }
        let used: Vec<usize> = group.iter().filter(|g| g.3 > s).map(|g| g.1).collect();
        let lane = (0..).find(|l| !used.contains(l)).unwrap();
        group.push((i, lane, s, e));
        group_end = group_end.max(e);
    }
    flush(&mut group, &mut out);
    out
}

/// Entries are assigned to the day they start on (`isSameDay(start, day)`).
pub fn entries_for_day(entries: &[TimeEntry], day: NaiveDate) -> Vec<&TimeEntry> {
    entries.iter().filter(|e| e.start.with_timezone(&Local).date_naive() == day).collect()
}

/// (start, end) minutes within `day`; an end at the next midnight is 1440.
pub fn entry_span_minutes(e: &TimeEntry, day: NaiveDate) -> (i64, i64) {
    (minutes_of_day(e.start.with_timezone(&Local), day), minutes_of_day(e.end.with_timezone(&Local), day))
}

/// Worked minutes for the day: Σ(end − start − breakMinutes).
pub fn day_total_minutes(entries: &[TimeEntry], day: NaiveDate) -> i64 {
    entries_for_day(entries, day).iter().map(|e| ((e.end - e.start).num_minutes() - e.break_minutes).max(0)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use track_core::model::TimeEntry;
    use chrono::{Local, NaiveDate, TimeZone};

    fn entry(id: &str, s: (u32, u32, u32), e: (u32, u32, u32), brk: i64) -> TimeEntry {
        let t = |(d, h, m): (u32, u32, u32)| Local.with_ymd_and_hms(2026, 11, d, h, m, 0).unwrap().to_utc();
        TimeEntry {
            id: id.into(), project_id: None, start: t(s), end: t(e), title: None, note: None, project: None,
            tags: vec![], external_event_id: None, external_event_source: None, break_minutes: brk, kind: Default::default(),
        }
    }

    #[test]
    fn layout_matches_ts() {
        // A 9:00–10:00, B 9:30–11:00, C 10:00–10:30, D 12:00–13:00 (given out of order)
        let spans = vec![(720, 780), (600, 630), (540, 600), (570, 660)]; // D, C, A, B
        let laid = layout_blocks(&spans, |s| *s);
        let get = |i: usize| laid.iter().find(|l| l.index == i).unwrap();
        assert_eq!((get(2).lane, get(2).lane_count), (0, 2)); // A
        assert_eq!((get(3).lane, get(3).lane_count), (1, 2)); // B
        assert_eq!((get(1).lane, get(1).lane_count), (0, 2)); // C
        assert_eq!((get(0).lane, get(0).lane_count), (0, 1)); // D
    }

    #[test]
    fn a_block_spreads_over_free_lanes_to_its_right() {
        // A 9–11 (lane 0), B 9–9:30 (lane 1), C 9–9:30 (lane 2), D 10–11 (lane 1; lane 2 is free then).
        let spans = vec![(540, 660), (540, 570), (540, 570), (600, 660)];
        let laid = layout_blocks(&spans, |s| *s);
        let get = |i: usize| laid.iter().find(|l| l.index == i).unwrap();
        assert_eq!((get(3).lane, get(3).span, get(3).lane_count), (1, 2, 3), "D takes lanes 1 and 2");
        assert_eq!(get(1).span, 1, "B: C is beside it");
        assert_eq!(get(2).span, 1, "C is in the last lane");
        assert_eq!(get(0).span, 1, "A: B and D are beside it");
    }

    #[test]
    fn entry_ending_at_midnight_belongs_to_start_day() {
        let e = entry("a", (12, 23, 0), (13, 0, 0), 0);
        let all = vec![e.clone(), entry("b", (13, 9, 0), (13, 10, 0), 15)];
        let d12 = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let d13 = NaiveDate::from_ymd_opt(2026, 11, 13).unwrap();
        assert_eq!(entries_for_day(&all, d12).iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["a"]);
        assert_eq!(entries_for_day(&all, d13).iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["b"]);
        assert_eq!(day_total_minutes(&all, d12), 60);
        assert_eq!(day_total_minutes(&all, d13), 45);
        assert_eq!(entry_span_minutes(&e, d12), (1380, 1440));
    }

    #[test]
    fn day_total_never_counts_negative_entries() {
        // 15-minute entry with a 60-minute break must count as 0, not -45.
        let all = vec![entry("a", (12, 9, 0), (12, 9, 15), 60), entry("b", (12, 10, 0), (12, 11, 0), 0)];
        assert_eq!(day_total_minutes(&all, NaiveDate::from_ymd_opt(2026, 11, 12).unwrap()), 60);
    }
}
