//! Where the arrow keys move the calendar's selection: ↑↓ through the entries in time order, ←→
//! to the block nearest on screen beside it. Works on the blocks as drawn (day column, start and
//! end in minutes, lane), not on the entries themselves.

#[derive(Debug, Clone, PartialEq)]
pub struct NavBlock {
    pub id: String,
    /// The column (shown day) it is drawn in.
    pub day: usize,
    pub start: i64,
    pub end: i64,
    /// Its lane among the blocks it overlaps on that day (0 = leftmost).
    pub lane: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// The block to select after pressing `dir` with `current` selected. With nothing selected,
/// the first block of `today` (a column index) when it has one, else the first block shown. At
/// the edge it stays where it is.
pub fn move_selection(blocks: &[NavBlock], current: Option<&str>, dir: Dir, today: Option<usize>) -> Option<String> {
    let mut order: Vec<&NavBlock> = blocks.iter().collect();
    order.sort_by(|a, b| (a.day, a.start, a.lane, a.end, &a.id).cmp(&(b.day, b.start, b.lane, b.end, &b.id)));
    let Some(cur) = current.and_then(|id| order.iter().position(|b| b.id == id)) else {
        let first = today.and_then(|t| order.iter().find(|b| b.day == t)).or(order.first());
        return first.map(|b| b.id.clone());
    };
    let c = order[cur];
    let picked = match dir {
        Dir::Down => order.get(cur + 1).copied(),
        Dir::Up => cur.checked_sub(1).map(|i| order[i]),
        Dir::Left | Dir::Right => beside(&order, c, dir == Dir::Right),
    };
    Some(picked.unwrap_or(c).id.clone())
}

fn overlaps(a: &NavBlock, b: &NavBlock) -> bool {
    a.start < b.end && b.start < a.end
}

/// Twice the distance between two blocks' vertical centres (kept whole).
fn centre_gap(a: &NavBlock, b: &NavBlock) -> i64 {
    ((a.start + a.end) - (b.start + b.end)).abs()
}

/// The block nearest beside `c` on screen: the next lane over in its own day while one overlaps
/// it, else the nearest block of the nearest day with any (overlapping first, by centre; then by
/// the gap above or below).
fn beside<'a>(blocks: &[&'a NavBlock], c: &NavBlock, right: bool) -> Option<&'a NavBlock> {
    let same_day = blocks
        .iter()
        .filter(|b| b.day == c.day && b.id != c.id && overlaps(b, c) && if right { b.lane > c.lane } else { b.lane < c.lane })
        .min_by_key(|b| (b.lane.abs_diff(c.lane), centre_gap(b, c)));
    if let Some(b) = same_day {
        return Some(*b);
    }
    let last = blocks.iter().map(|b| b.day).max()?;
    let mut day = c.day;
    loop {
        day = if right {
            if day >= last {
                return None;
            }
            day + 1
        } else {
            day.checked_sub(1)?
        };
        let in_day: Vec<&NavBlock> = blocks.iter().copied().filter(|b| b.day == day).collect();
        if in_day.is_empty() {
            continue;
        }
        let near = in_day.iter().filter(|b| overlaps(b, c)).min_by_key(|b| (centre_gap(b, c), b.lane));
        let gap = |b: &NavBlock| if b.end <= c.start { c.start - b.end } else { b.start - c.end };
        return near.or_else(|| in_day.iter().min_by_key(|b| (gap(b), b.start))).copied();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(id: &str, day: usize, start: i64, end: i64, lane: usize) -> NavBlock {
        NavBlock { id: id.into(), day, start, end, lane }
    }
    fn go(blocks: &[NavBlock], from: Option<&str>, dir: Dir) -> Option<String> {
        move_selection(blocks, from, dir, None)
    }

    #[test]
    fn up_and_down_follow_time_order_across_days() {
        // Mon: a 9–10, b 9–10 beside it (lane 1), c 11–12. Wed: d 8–9.
        let blocks = [b("c", 0, 660, 720, 0), b("b", 0, 540, 600, 1), b("a", 0, 540, 600, 0), b("d", 2, 480, 540, 0)];
        assert_eq!(go(&blocks, Some("a"), Dir::Down).as_deref(), Some("b"), "same start: left lane first");
        assert_eq!(go(&blocks, Some("b"), Dir::Down).as_deref(), Some("c"));
        assert_eq!(go(&blocks, Some("c"), Dir::Down).as_deref(), Some("d"), "on to the next day with entries");
        assert_eq!(go(&blocks, Some("d"), Dir::Down).as_deref(), Some("d"), "stops at the end");
        assert_eq!(go(&blocks, Some("d"), Dir::Up).as_deref(), Some("c"));
        assert_eq!(go(&blocks, Some("a"), Dir::Up).as_deref(), Some("a"), "stops at the start");
    }

    #[test]
    fn nothing_selected_starts_today_or_at_the_first() {
        let blocks = [b("late", 3, 600, 660, 0), b("mon", 0, 600, 660, 0), b("tue2", 1, 700, 760, 0), b("tue1", 1, 540, 600, 0)];
        assert_eq!(move_selection(&blocks, None, Dir::Down, Some(1)).as_deref(), Some("tue1"), "today's first");
        assert_eq!(move_selection(&blocks, None, Dir::Right, Some(2)).as_deref(), Some("mon"), "today has none: the first shown");
        assert_eq!(move_selection(&blocks, None, Dir::Up, None).as_deref(), Some("mon"));
        assert_eq!(move_selection(&[], None, Dir::Down, None), None);
    }

    #[test]
    fn left_and_right_go_beside_in_the_same_day_first() {
        // Mon: a 9–11 (lane 0), b 10–12 (lane 1), c 10–10:30 (lane 2). Tue: t 10–11.
        let blocks = [b("a", 0, 540, 660, 0), b("b", 0, 600, 720, 1), b("c", 0, 600, 630, 2), b("t", 1, 600, 660, 0)];
        assert_eq!(go(&blocks, Some("a"), Dir::Right).as_deref(), Some("b"), "the next lane beside it");
        assert_eq!(go(&blocks, Some("b"), Dir::Right).as_deref(), Some("c"));
        assert_eq!(go(&blocks, Some("c"), Dir::Right).as_deref(), Some("t"), "then the next day");
        assert_eq!(go(&blocks, Some("c"), Dir::Left).as_deref(), Some("b"));
        assert_eq!(go(&blocks, Some("a"), Dir::Left).as_deref(), Some("a"), "stops at the left edge");
    }

    #[test]
    fn left_and_right_pick_the_nearest_block_on_screen() {
        // From x 10–11 (centre 10:30) on Mon. Tue: long 9–18, short 10:15–10:45, far 14–15.
        let blocks = [b("x", 0, 600, 660, 0), b("long", 1, 540, 1080, 0), b("short", 1, 615, 645, 1), b("far", 1, 840, 900, 0)];
        assert_eq!(go(&blocks, Some("x"), Dir::Right).as_deref(), Some("short"), "overlapping, centre nearest");
        // Nothing overlaps: the smallest gap. From y 9–10 on Thu: Fri has 11–12 (60 below) and
        // 6:00–8:20 (40 above).
        let gaps = [b("y", 3, 540, 600, 0), b("after", 4, 660, 720, 0), b("before", 4, 360, 500, 0)];
        assert_eq!(go(&gaps, Some("y"), Dir::Right).as_deref(), Some("before"), "gap 40 beats gap 60");
        // Empty days are skipped.
        let skip = [b("m", 0, 600, 660, 0), b("w", 2, 600, 660, 0)];
        assert_eq!(go(&skip, Some("m"), Dir::Right).as_deref(), Some("w"));
        assert_eq!(go(&skip, Some("w"), Dir::Left).as_deref(), Some("m"));
        assert_eq!(go(&skip, Some("w"), Dir::Right).as_deref(), Some("w"), "stops at the right edge");
    }
}
