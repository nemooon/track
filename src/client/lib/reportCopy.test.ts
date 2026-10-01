import { describe, expect, test } from "bun:test";
import {
  buildAggregationCopyText,
  buildReportCopyText,
  formatReportDuration,
  reportCopyFormatUsesAi,
  reorderReportCopyColumns,
} from "./reportCopy";
import type { ReportEntry } from "@shared/types";

const entry: ReportEntry = {
  id: "entry-1",
  start: "2026-08-04T01:00:00.000Z",
  end: "2026-08-04T02:30:00.000Z",
  minutes: 90,
  title: "設計, レビュー",
  note: "",
  project: {
    id: "project-1",
    name: "Track",
    color: "#000000",
    client: { id: "client-1", name: "社内" },
  },
  tags: [{ id: "tag-1", name: "開発", color: "#000000" }],
};

describe("report copy formatter", () => {
  test("全体指示またはAI生成項目があるフォーマットでAIを使用する", () => {
    const format = {
      id: "standard",
      name: "標準",
      target: "ai-aggregation" as const,
      delimiter: "tab" as const,
      includeHeader: true,
      aiPrompt: "",
      columns: [{ id: "category", kind: "field" as const, field: "category" as const }],
    };
    expect(reportCopyFormatUsesAi(format)).toBe(false);
    expect(reportCopyFormatUsesAi({ ...format, aiPrompt: "成果別に分類する" })).toBe(true);
    expect(reportCopyFormatUsesAi({
      ...format,
      columns: [
        ...format.columns,
        {
          id: "outcome",
          kind: "ai" as const,
          label: "成果",
          prompt: "成果を要約する",
        },
      ],
    })).toBe(true);
    expect(reportCopyFormatUsesAi({
      ...format,
      columns: [{ id: "summary", kind: "field" as const, field: "summary" as const }],
    })).toBe(true);
  });

  test("時間を選択した表示形式へ変換する", () => {
    expect(formatReportDuration(90, "hours-minutes")).toBe("1h 30m");
    expect(formatReportDuration(90, "japanese")).toBe("1時間30分");
    expect(formatReportDuration(90, "decimal-with-unit")).toBe("1.5時間");
    expect(formatReportDuration(90, "decimal")).toBe("1.5");
    expect(formatReportDuration(80, "decimal")).toBe("1.33");
  });

  test("同じ時間項目でも列ごとに表示形式を変えられる", () => {
    expect(
      buildReportCopyText([entry], 90, {
        id: "duration-formats",
        name: "時間形式",
        target: "entries",
        delimiter: "tab",
        includeHeader: false,
        aiPrompt: "",
        columns: [
          {
            id: "duration-ja",
            kind: "field",
            field: "duration",
            durationFormat: "japanese",
          },
          {
            id: "duration-decimal",
            kind: "field",
            field: "duration",
            durationFormat: "decimal",
          },
        ],
      }),
    ).toBe("1時間30分\t1.5");
  });

  test("指定した順番とタブ区切りで出力する", () => {
    expect(
      buildReportCopyText([entry], 180, {
        id: "slack",
        name: "Slack用",
        target: "entries",
        delimiter: "tab",
        includeHeader: true,
        aiPrompt: "",
        columns: ["project", "title", "duration", "percentage"].map(
          (field, index) => ({
            id: `column-${index}`,
            kind: "field" as const,
            field: field as "project" | "title" | "duration" | "percentage",
            ...(field === "duration"
              ? { durationFormat: "hours-minutes" as const }
              : {}),
          }),
        ),
      }),
    ).toBe("プロジェクト\t内容\t時間\t割合\nTrack\t設計, レビュー\t1h 30m\t50%");
  });

  test("CSVのカンマと引用符をエスケープする", () => {
    expect(
      buildReportCopyText([entry], 90, {
        id: "csv",
        name: "CSV",
        target: "entries",
        delimiter: "comma",
        includeHeader: false,
        aiPrompt: "",
        columns: [
          { id: "title-1", kind: "field", field: "title" },
          { id: "minutes", kind: "field", field: "durationMinutes" },
        ],
      }),
    ).toBe('"設計, レビュー",90');
  });

  test("同じ項目を複数の列へ出力する", () => {
    expect(
      buildReportCopyText([entry], 90, {
        id: "duplicate",
        name: "重複列",
        target: "entries",
        delimiter: "tab",
        includeHeader: true,
        aiPrompt: "",
        columns: [
          { id: "project-1", kind: "field", field: "project" },
          { id: "project-2", kind: "field", field: "project" },
        ],
      }),
    ).toBe("プロジェクト\tプロジェクト\nTrack\tTrack");
  });

  test("見出しを変更し、空白項目は常に空文字で出力する", () => {
    expect(
      buildReportCopyText([entry], 90, {
        id: "custom-header",
        name: "見出し変更",
        target: "entries",
        delimiter: "tab",
        includeHeader: true,
        aiPrompt: "",
        columns: [
          { id: "project", kind: "field", field: "project", label: "案件" },
          { id: "blank", kind: "blank", label: "" },
          {
            id: "duration",
            kind: "field",
            field: "duration",
            label: "工数",
            durationFormat: "hours-minutes",
          },
        ],
      }),
    ).toBe("案件\t\t工数\nTrack\t\t1h 30m");
  });

  test("AI生成項目を含む集計結果を出力する", () => {
    expect(
      buildAggregationCopyText(
        [{
          label: "開発",
          summary: "機能実装",
          minutes: 90,
          entryCount: 2,
          generatedValues: { outcome: "コピー機能を改善" },
        }],
        90,
        {
          id: "ai",
          name: "AI使用の出力",
          target: "ai-aggregation",
          delimiter: "tab",
          includeHeader: true,
          aiPrompt: "成果が同じ作業を分類する",
          columns: [
            { id: "category", kind: "field", field: "category" },
            { id: "outcome", kind: "ai", label: "成果", prompt: "成果を要約" },
          ],
        },
      ),
    ).toBe("集計名\t成果\n開発\tコピー機能を改善");
  });

  test("指定した位置へ列を並べ替える", () => {
    const columns = [
      { id: "a", kind: "field" as const, field: "category" as const },
      { id: "b", kind: "field" as const, field: "summary" as const },
      { id: "c", kind: "field" as const, field: "duration" as const },
    ];
    expect(reorderReportCopyColumns(columns, "a", "c").map((column) => column.id))
      .toEqual(["b", "c", "a"]);
    expect(reorderReportCopyColumns(columns, "c", "a").map((column) => column.id))
      .toEqual(["c", "a", "b"]);
  });
});
