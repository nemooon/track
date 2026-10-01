import { describe, expect, test } from "bun:test";
import {
  buildReportOutputCacheKey,
  readReportOutputCache,
  writeReportOutputCache,
} from "./reportOutputCache";
import type { ReportCopyFormat, ReportEntry } from "@shared/types";

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() { return values.size; },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => { values.delete(key); },
    setItem: (key, value) => { values.set(key, value); },
  };
}

const copyFormat: ReportCopyFormat = {
  id: "standard",
  name: "標準",
  target: "ai-aggregation",
  delimiter: "tab",
  includeHeader: true,
  aiPrompt: "目的別に分類する",
  columns: [{ id: "category", kind: "field", field: "category" }],
};

function cacheKey(overrides: Partial<Parameters<typeof buildReportOutputCacheKey>[0]> = {}) {
  return buildReportOutputCacheKey({
    range: "month",
    anchor: "2026-08-01",
    groupBy: "project",
    clientIds: ["client-b", "client-a"],
    projectIds: [],
    tagIds: [],
    reportRows: [{ key: "project", label: "案件", totalMinutes: 90 }],
    totalMinutes: 90,
    entries: [],
    copyFormat,
    ...overrides,
  });
}

describe("report output cache", () => {
  test("同じ条件では選択IDの順番にかかわらず同じキーになる", () => {
    expect(cacheKey()).toBe(cacheKey({ clientIds: ["client-a", "client-b"] }));
  });

  test("合計時間が同じでも記録の本文が変われば別のキーになる", () => {
    const entry: ReportEntry = {
      id: "entry", start: "2026-08-01T01:00:00Z", end: "2026-08-01T02:30:00Z",
      minutes: 90, title: "実装", note: "認証", project: null, tags: [],
    };
    expect(cacheKey({ entries: [entry] })).not.toBe(
      cacheKey({ entries: [{ ...entry, note: "決済" }] }),
    );
  });

  test("フォーマットが変わると別のキーになる", () => {
    expect(cacheKey()).not.toBe(
      cacheKey({ copyFormat: { ...copyFormat, delimiter: "comma" } }),
    );
  });

  test("保存した結果を読み込み、7日を過ぎた結果は復元しない", () => {
    const storage = memoryStorage();
    const key = cacheKey();
    const createdAt = Date.UTC(2026, 7, 4);
    writeReportOutputCache(storage, {
      key,
      createdAt,
      totalMinutes: 90,
      rows: [{
        label: "開発",
        summary: "実装",
        minutes: 90,
        entryCount: 2,
        generatedValues: {},
      }],
    }, createdAt);

    expect(readReportOutputCache(storage, key, createdAt)?.rows[0].label).toBe("開発");
    expect(readReportOutputCache(storage, key, createdAt + 8 * 24 * 60 * 60 * 1_000)).toBeNull();
  });
});
