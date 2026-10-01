import { describe, expect, test } from "bun:test";
import { parseReportCopyFormats } from "./settings";
import type { ReportCopyFormat } from "@shared/types";

describe("report output settings", () => {
  test("旧来の直接コピー形式を標準の出力フォーマットへ置き換える", () => {
    const formats = parseReportCopyFormats(
      JSON.stringify([
        {
          id: "slack",
          name: "Slack用",
          delimiter: "tab",
          includeHeader: true,
          fields: ["project", "project", "duration"],
        },
      ]),
    );

    expect(formats).toHaveLength(1);
    expect(formats[0].target).toBe("ai-aggregation");
    expect(formats[0].columns.map((column) =>
      column.kind === "field" ? column.field : "ai",
    )).toEqual(["category", "duration", "percentage"]);
    expect(formats[0].aiPrompt).toBe("");
  });

  test("同じ項目を持つ新形式をそのまま読み込む", () => {
    const source: ReportCopyFormat[] = [{
      id: "duplicate",
      name: "重複",
      target: "ai-aggregation",
      delimiter: "comma",
      includeHeader: true,
      aiPrompt: "",
      columns: [
        { id: "one", kind: "field", field: "category", label: "分類1" },
        { id: "blank", kind: "blank", label: "" },
        { id: "two", kind: "field", field: "category" },
        {
          id: "duration",
          kind: "field",
          field: "duration",
          durationFormat: "decimal",
        },
      ],
    }];
    expect(parseReportCopyFormats(JSON.stringify(source))).toEqual(source);
  });

  test("既存形式の時間表示を時間項目へ移行する", () => {
    const [format] = parseReportCopyFormats(JSON.stringify([{
      id: "existing",
      name: "既存",
      target: "ai-aggregation",
      delimiter: "tab",
      durationFormat: "decimal-with-unit",
      includeHeader: true,
      aiPrompt: "",
      columns: [{ id: "duration", kind: "field", field: "duration" }],
    }]));

    const durationColumn = format.columns.find(
      (column) => column.kind === "field" && column.field === "duration",
    );
    expect(
      durationColumn?.kind === "field"
        ? durationColumn.durationFormat
        : undefined,
    ).toBe("decimal-with-unit");
    expect("durationFormat" in format).toBe(false);
  });

  test("既存のAI生成列がある形式にはAIへの指示を補う", () => {
    const [format] = parseReportCopyFormats(JSON.stringify([{
      id: "existing-ai",
      name: "顧客報告",
      target: "ai-aggregation",
      delimiter: "tab",
      includeHeader: true,
      columns: [
        { id: "summary", kind: "field", field: "summary" },
      ],
    }]));

    expect(format.aiPrompt).not.toBe("");
  });

  test("全体指示が空でもAI生成項目を保存できる", () => {
    const source: ReportCopyFormat[] = [{
      id: "generated-column-only",
      name: "生成項目のみ",
      target: "ai-aggregation",
      delimiter: "tab",
      includeHeader: true,
      aiPrompt: "",
      columns: [
        { id: "category", kind: "field", field: "category" },
        {
          id: "outcome",
          kind: "ai",
          label: "成果",
          prompt: "成果を簡潔にまとめる",
        },
      ],
    }];

    expect(parseReportCopyFormats(JSON.stringify(source))).toEqual(source);
  });
});
