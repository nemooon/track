//! Calendar pointer state machine (WeekCalendar.tsx `interaction`), free of GPUI.
use super::time::{DAY_MINUTES, SNAP_MIN};
use track_core::model::TimeEntry;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DragState {
    #[default]
    Idle,
    Creating { day_index: usize, anchor_min: i64, current_min: i64 },
    Picking { day_index: usize, start_min: i64, end_min: i64 },
    Moving { id: String, day_index: usize, start_min: i64, duration: i64, offset_min: i64, orig_day_index: usize, orig_start_min: i64 },
    Resizing { id: String, day_index: usize, edge: Edge, start_min: i64, end_min: i64, orig: (i64, i64) },
}

pub fn begin_create(day_index: usize, min: i64) -> DragState {
    DragState::Creating { day_index, anchor_min: min, current_min: min }
}

pub fn update_create(s: &DragState, min: i64) -> DragState {
    match s {
        DragState::Creating { day_index, anchor_min, .. } => {
            DragState::Creating { day_index: *day_index, anchor_min: *anchor_min, current_min: min.clamp(0, DAY_MINUTES) }
        }
        other => other.clone(),
    }
}

/// Pointer-up while creating: a span under 15 minutes is cancelled.
pub fn finish_create(s: &DragState) -> Option<DragState> {
    let DragState::Creating { day_index, anchor_min, current_min } = *s else { return None };
    let (start_min, end_min) = (anchor_min.min(current_min), anchor_min.max(current_min));
    (end_min - start_min >= SNAP_MIN).then_some(DragState::Picking { day_index, start_min, end_min })
}

/// `hit_day`: the column under the pointer, or None outside all columns (keeps the current day).
/// `pointer_min`: snapped minutes under the pointer in that column.
pub fn update_move(s: &DragState, hit_day: Option<usize>, pointer_min: i64) -> DragState {
    let DragState::Moving { id, day_index, duration, offset_min, orig_day_index, orig_start_min, .. } = s else { return s.clone() };
    let start = (pointer_min - offset_min).clamp(0, DAY_MINUTES).min(DAY_MINUTES - duration).max(0);
    DragState::Moving {
        id: id.clone(),
        day_index: hit_day.unwrap_or(*day_index),
        start_min: start,
        duration: *duration,
        offset_min: *offset_min,
        orig_day_index: *orig_day_index,
        orig_start_min: *orig_start_min,
    }
}

pub fn update_resize(s: &DragState, pointer_min: i64) -> DragState {
    let DragState::Resizing { id, day_index, edge, start_min, end_min, orig } = s else { return s.clone() };
    let m = pointer_min.clamp(0, DAY_MINUTES);
    let (start_min, end_min) = match edge {
        Edge::Top => (m.min(end_min - SNAP_MIN).max(0), *end_min),
        Edge::Bottom => (*start_min, m.max(start_min + SNAP_MIN)),
    };
    DragState::Resizing { id: id.clone(), day_index: *day_index, edge: *edge, start_min, end_min, orig: *orig }
}

/// Arrow-key nudge. Plain: shift both ends, refused outside 0:00–24:00.
/// With Shift: move the end only, refused below 15 minutes.
pub fn nudge(start: i64, end: i64, delta: i64, shift: bool) -> Option<(i64, i64)> {
    if shift {
        let e = end + delta;
        (e - start >= SNAP_MIN).then_some((start, e))
    } else {
        let (s, e) = (start + delta, end + delta);
        (s >= 0 && e <= DAY_MINUTES).then_some((s, e))
    }
}

/// The calendar's entry list with optimistic edits and snapshot rollback.
#[derive(Debug, Clone, Default)]
pub struct EntryCache {
    entries: Vec<TimeEntry>,
}

impl EntryCache {
    pub fn new(entries: Vec<TimeEntry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[TimeEntry] {
        &self.entries
    }

    pub fn get(&self, id: &str) -> Option<&TimeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn snapshot(&self) -> Vec<TimeEntry> {
        self.entries.clone()
    }

    pub fn rollback(&mut self, snapshot: Vec<TimeEntry>) {
        self.entries = snapshot;
    }

    pub fn apply(&mut self, id: &str, start: DateTime<Utc>, end: DateTime<Utc>) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.start = start;
            e.end = end;
        }
    }

    pub fn replace(&mut self, updated: TimeEntry) {
        match self.entries.iter_mut().find(|e| e.id == updated.id) {
            Some(e) => *e = updated,
            None => self.insert(updated),
        }
    }

    /// Insert keeping start order (createEntry onSuccess).
    pub fn insert(&mut self, entry: TimeEntry) {
        self.entries.retain(|e| e.id != entry.id);
        self.entries.push(entry);
        self.entries.sort_by_key(|e| e.start);
    }

    pub fn remove(&mut self, id: &str) -> Option<TimeEntry> {
        let i = self.entries.iter().position(|e| e.id == id)?;
        Some(self.entries.remove(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::entry;

    #[test]
    fn create_under_15_minutes_is_cancelled() {
        let s = begin_create(2, 600);
        assert_eq!(finish_create(&update_create(&s, 605)), None);
        assert_eq!(finish_create(&update_create(&s, 615)), Some(DragState::Picking { day_index: 2, start_min: 600, end_min: 615 }));
        assert_eq!(finish_create(&update_create(&s, 540)), Some(DragState::Picking { day_index: 2, start_min: 540, end_min: 600 }));
    }

    #[test]
    fn move_clamps_to_day_bounds_and_columns() {
        // 60-minute entry grabbed 15 minutes below its start.
        let s = DragState::Moving { id: "a".into(), day_index: 1, start_min: 600, duration: 60, offset_min: 15, orig_day_index: 1, orig_start_min: 600 };
        assert_eq!(update_move(&s, Some(3), 1440), DragState::Moving { id: "a".into(), day_index: 3, start_min: 1380, duration: 60, offset_min: 15, orig_day_index: 1, orig_start_min: 600 });
        let DragState::Moving { day_index, start_min, .. } = update_move(&s, None, 0) else { panic!() };
        assert_eq!((day_index, start_min), (1, 0)); // outside any column keeps the current day
        let DragState::Moving { start_min, .. } = update_move(&s, Some(1), 700) else { panic!() };
        assert_eq!(start_min, 685);
    }

    #[test]
    fn resize_keeps_min_15() {
        let top = DragState::Resizing { id: "a".into(), day_index: 0, edge: Edge::Top, start_min: 600, end_min: 660, orig: (600, 660) };
        let DragState::Resizing { start_min, .. } = update_resize(&top, 700) else { panic!() };
        assert_eq!(start_min, 645);
        let bottom = DragState::Resizing { id: "a".into(), day_index: 0, edge: Edge::Bottom, start_min: 600, end_min: 660, orig: (600, 660) };
        let DragState::Resizing { end_min, .. } = update_resize(&bottom, 600) else { panic!() };
        assert_eq!(end_min, 615);
        let DragState::Resizing { end_min, .. } = update_resize(&bottom, 1440) else { panic!() };
        assert_eq!(end_min, 1440);
    }

    #[test]
    fn nudge_stays_within_day() {
        assert_eq!(nudge(0, 60, -15, false), None);
        assert_eq!(nudge(1380, 1440, 15, false), None);
        assert_eq!(nudge(600, 660, 15, false), Some((615, 675)));
        assert_eq!(nudge(600, 615, -15, true), None);
        assert_eq!(nudge(600, 615, 15, true), Some((600, 630)));
    }

    #[test]
    fn failed_update_rolls_back() {
        let mut cache = EntryCache::new(vec![entry("a", 12, (9, 0), (10, 0), None), entry("b", 12, (11, 0), (12, 0), None)]);
        let snapshot = cache.snapshot();
        let moved = entry("a", 13, (9, 0), (10, 0), None);
        cache.apply(&moved.id, moved.start, moved.end);
        assert_eq!(cache.get("a").unwrap().start, moved.start);
        cache.rollback(snapshot.clone());
        assert_eq!(cache.entries(), &snapshot[..]);
        let removed = cache.remove("b").unwrap();
        assert_eq!(removed.id, "b");
        assert_eq!(cache.entries().len(), 1);
    }
}
