import type { ReportCopyFormat, ReportRow, ReportEntry } from "@shared/types";
import type { AiAggregationRow } from "./reportAggregation";

const STORAGE_KEY = "reports.customOutputCache.v1";
const MAX_CACHE_ITEMS = 8;
const MAX_CACHE_AGE_MS = 7 * 24 * 60 * 60 * 1_000;

type CacheEntry = {
  key: string;
  createdAt: number;
  totalMinutes: number;
  rows: AiAggregationRow[];
};

export function buildReportOutputCacheKey(input: {
  range: "week" | "month";
  anchor: string;
  groupBy: "client" | "project" | "tag";
  clientIds: string[];
  projectIds: string[];
  tagIds: string[];
  reportRows: ReportRow[];
  entries: ReportEntry[];
  totalMinutes: number;
  copyFormat: ReportCopyFormat;
}): string {
  return JSON.stringify({
    version: 2,
    entries: [...input.entries].sort((a, b) => a.id.localeCompare(b.id)),
    range: input.range,
    anchor: input.anchor,
    groupBy: input.groupBy,
    clientIds: [...input.clientIds].sort(),
    projectIds: [...input.projectIds].sort(),
    tagIds: [...input.tagIds].sort(),
    reportRows: input.reportRows.map((row) => ({
      key: row.key,
      label: row.label,
      totalMinutes: row.totalMinutes,
    })),
    totalMinutes: input.totalMinutes,
    copyFormat: input.copyFormat,
  });
}

function validRows(value: unknown): value is AiAggregationRow[] {
  return (
    Array.isArray(value) &&
    value.every(
      (row) =>
        row &&
        typeof row === "object" &&
        typeof row.label === "string" &&
        typeof row.summary === "string" &&
        typeof row.minutes === "number" &&
        typeof row.entryCount === "number" &&
        row.generatedValues &&
        typeof row.generatedValues === "object",
    )
  );
}

function readEntries(storage: Storage, now: number): CacheEntry[] {
  try {
    const parsed = JSON.parse(storage.getItem(STORAGE_KEY) ?? "[]") as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (entry): entry is CacheEntry =>
        Boolean(entry) &&
        typeof entry === "object" &&
        typeof entry.key === "string" &&
        typeof entry.createdAt === "number" &&
        now - entry.createdAt <= MAX_CACHE_AGE_MS &&
        typeof entry.totalMinutes === "number" &&
        validRows(entry.rows),
    );
  } catch {
    return [];
  }
}

export function readReportOutputCache(
  storage: Storage,
  key: string,
  now = Date.now(),
): Omit<CacheEntry, "key"> | null {
  const entry = readEntries(storage, now).find((item) => item.key === key);
  return entry
    ? {
        createdAt: entry.createdAt,
        totalMinutes: entry.totalMinutes,
        rows: entry.rows,
      }
    : null;
}

export function writeReportOutputCache(
  storage: Storage,
  entry: CacheEntry,
  now = Date.now(),
) {
  const entries = readEntries(storage, now).filter(
    (item) => item.key !== entry.key,
  );
  try {
    storage.setItem(
      STORAGE_KEY,
      JSON.stringify([entry, ...entries].slice(0, MAX_CACHE_ITEMS)),
    );
  } catch {
    // 容量制限やプライベートモードではキャッシュせず続行する
  }
}
