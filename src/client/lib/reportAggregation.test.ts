import { describe, expect, test } from "bun:test";
import {
  aggregateAiResult,
  buildReportAggregationPrompt,
} from "./reportAggregation";
import type { ReportEntriesResponse } from "@shared/types";

const data: ReportEntriesResponse = {
  totalMinutes: 90,
  entries: [
    {
      id: "a",
      start: "2026-08-04T01:00:00.000Z",
      end: "2026-08-04T02:00:00.000Z",
      minutes: 60,
      title: "実装",
      note: null,
      project: null,
      tags: [],
    },
    {
      id: "b",
      start: "2026-08-04T02:00:00.000Z",
      end: "2026-08-04T02:30:00.000Z",
      minutes: 30,
      title: "レビュー",
      note: null,
      project: null,
      tags: [],
    },
  ],
};

describe("AI report aggregation", () => {
  test("AIの分類IDを使い、元データの時間を合算する", () => {
    const rows = aggregateAiResult(
      '```json\n{"groups":[{"label":"開発","summary":"実装と確認","entryIds":["a","b"],"values":[{"id":"outcome","value":"機能を完成"}]}]}\n```',
      data,
    );
    expect(rows).toEqual([
      {
        label: "開発",
        summary: "実装と確認",
        minutes: 90,
        entryCount: 2,
        generatedValues: { outcome: "機能を完成" },
      },
    ]);
  });

  test("未分類の記録をその他へ入れ、重複IDを数えない", () => {
    const rows = aggregateAiResult(
      '{"groups":[{"label":"実装","entryIds":["a","a","unknown"]}]}',
      data,
    );
    expect(rows.reduce((sum, row) => sum + row.minutes, 0)).toBe(90);
    expect(rows.find((row) => row.label === "その他")?.minutes).toBe(30);
  });

  test("AI生成列の順番と指示をプロンプトへ含める", () => {
    const prompt = buildReportAggregationPrompt(data, {
      id: "ai",
      name: "顧客報告",
      target: "ai-aggregation",
      delimiter: "comma",
      includeHeader: true,
      aiPrompt: "目的別にまとめる",
      columns: [
        { id: "category", kind: "field", field: "category", label: "区分" },
        { id: "spacer", kind: "blank", label: "" },
        { id: "outcome", kind: "ai", label: "成果", prompt: "成果を20文字以内で要約" },
      ],
    });
    expect(prompt).toContain("形式名: 顧客報告");
    expect(prompt).toContain("目的別にまとめる");
    expect(prompt).toContain('"label":"区分"');
    expect(prompt).toContain('"label":"成果"');
    expect(prompt).toContain("成果を20文字以内で要約");
    expect(prompt).not.toContain('"id":"spacer"');
  });
});
