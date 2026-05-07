import type { TimeEntry } from "@/types";

export type LaidOut<T> = {
  item: T;
  laneIndex: number;
  laneCount: number;
};

export type LaidOutEntry = {
  entry: TimeEntry;
  laneIndex: number;
  laneCount: number;
};

/**
 * Sweep-line lane assignment for overlapping time blocks within a single day.
 * Groups overlapping blocks together; within a group all blocks share laneCount.
 */
export function layoutBlocks<T extends { start: string; end: string }>(blocks: T[]): LaidOut<T>[] {
  if (blocks.length === 0) return [];
  const sorted = [...blocks].sort((a, b) => {
    const s = new Date(a.start).getTime() - new Date(b.start).getTime();
    if (s !== 0) return s;
    return new Date(b.end).getTime() - new Date(a.end).getTime();
  });

  const result: LaidOut<T>[] = [];
  let group: { item: T; laneIndex: number; endMs: number }[] = [];
  let groupEnd = 0;

  const flush = () => {
    const laneCount = group.reduce((m, g) => Math.max(m, g.laneIndex + 1), 0);
    for (const g of group) {
      result.push({ item: g.item, laneIndex: g.laneIndex, laneCount });
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
    const usedLanes = new Set(group.filter((g) => g.endMs > startMs).map((g) => g.laneIndex));
    let lane = 0;
    while (usedLanes.has(lane)) lane++;
    group.push({ item: e, laneIndex: lane, endMs });
    groupEnd = Math.max(groupEnd, endMs);
  }
  flush();
  return result;
}

export function layoutEntries(entries: TimeEntry[]): LaidOutEntry[] {
  return layoutBlocks(entries).map(({ item, laneIndex, laneCount }) => ({
    entry: item,
    laneIndex,
    laneCount,
  }));
}
