import type { TimeEntry } from "@/types";

export type LaidOutEntry = {
  entry: TimeEntry;
  laneIndex: number;
  laneCount: number;
};

/**
 * Sweep-line lane assignment for overlapping entries within a single day.
 * Groups overlapping entries together; within a group all entries share laneCount.
 */
export function layoutEntries(entries: TimeEntry[]): LaidOutEntry[] {
  if (entries.length === 0) return [];
  const sorted = [...entries].sort((a, b) => {
    const s = new Date(a.start).getTime() - new Date(b.start).getTime();
    if (s !== 0) return s;
    return new Date(b.end).getTime() - new Date(a.end).getTime();
  });

  const result: LaidOutEntry[] = [];
  let group: { entry: TimeEntry; laneIndex: number; endMs: number }[] = [];
  let groupEnd = 0;

  const flush = () => {
    const laneCount = group.reduce((m, g) => Math.max(m, g.laneIndex + 1), 0);
    for (const g of group) {
      result.push({ entry: g.entry, laneIndex: g.laneIndex, laneCount });
    }
    group = [];
    groupEnd = 0;
  };

  for (const e of sorted) {
    const startMs = new Date(e.start).getTime();
    const endMs = new Date(e.end).getTime();
    if (group.length && startMs >= groupEnd) {
      flush();
    }
    // find lowest free lane
    const usedLanes = new Set(group.filter((g) => g.endMs > startMs).map((g) => g.laneIndex));
    let lane = 0;
    while (usedLanes.has(lane)) lane++;
    group.push({ entry: e, laneIndex: lane, endMs });
    groupEnd = Math.max(groupEnd, endMs);
  }
  flush();
  return result;
}
