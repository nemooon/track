import { format } from "date-fns";
import type {
  ReportCopyColumn,
  ReportCopyField,
  ReportCopyFormat,
  ReportDurationFormat,
  ReportEntry,
} from "@shared/types";
import type { AiAggregationRow } from "./reportAggregation";

export const REPORT_COPY_FIELD_LABELS: Record<ReportCopyField, string> = {
  date: "日付",
  start: "開始",
  end: "終了",
  client: "クライアント",
  project: "プロジェクト",
  title: "内容",
  note: "メモ",
  tags: "タグ",
  duration: "時間",
  durationMinutes: "分数",
  percentage: "割合",
  category: "集計名",
  summary: "概要",
  entryCount: "件数",
};

export const ENTRY_REPORT_COPY_FIELDS: ReportCopyField[] = [
  "date", "start", "end", "client", "project", "title", "note", "tags",
  "duration", "durationMinutes", "percentage",
];

export const AI_REPORT_COPY_FIELDS: ReportCopyField[] = [
  "category", "summary", "duration", "durationMinutes", "percentage", "entryCount",
];

export function reportCopyFormatUsesAi(copyFormat: ReportCopyFormat): boolean {
  return (
    copyFormat.aiPrompt.trim().length > 0 ||
    copyFormat.columns.some(
      (column) =>
        column.kind === "ai" ||
        (column.kind === "field" && column.field === "summary"),
    )
  );
}

export function reportCopyColumnLabel(column: ReportCopyColumn): string {
  if (column.kind === "blank") return "空白";
  return column.kind === "ai" ? "AI生成" : REPORT_COPY_FIELD_LABELS[column.field];
}

export function reportCopyColumnHeader(column: ReportCopyColumn): string {
  if (column.kind === "ai") return column.label;
  if (column.label !== undefined) return column.label;
  return column.kind === "blank" ? "" : REPORT_COPY_FIELD_LABELS[column.field];
}

export function reorderReportCopyColumns(
  columns: ReportCopyColumn[],
  columnId: string,
  targetId: string,
): ReportCopyColumn[] {
  if (columnId === targetId) return columns;
  const from = columns.findIndex((column) => column.id === columnId);
  const to = columns.findIndex((column) => column.id === targetId);
  if (from === -1 || to === -1) return columns;
  const next = [...columns];
  const [column] = next.splice(from, 1);
  next.splice(to, 0, column);
  return next;
}

export function formatReportDuration(
  minutes: number,
  durationFormat: ReportDurationFormat,
): string {
  if (durationFormat === "decimal" || durationFormat === "decimal-with-unit") {
    const decimalHours = Number((minutes / 60).toFixed(2)).toString();
    return durationFormat === "decimal-with-unit"
      ? `${decimalHours}時間`
      : decimalHours;
  }
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (durationFormat === "japanese") {
    if (hours === 0) return `${rest}分`;
    return rest > 0 ? `${hours}時間${rest}分` : `${hours}時間`;
  }
  return rest > 0 ? `${hours}h ${rest}m` : `${hours}h`;
}

function entryColumnValue(
  entry: ReportEntry,
  column: ReportCopyColumn,
  totalMinutes: number,
): string {
  if (column.kind !== "field") return "";
  const start = new Date(entry.start);
  const end = new Date(entry.end);
  switch (column.field) {
    case "date":
      return format(start, "yyyy/MM/dd");
    case "start":
      return format(start, "HH:mm");
    case "end":
      return format(end, "HH:mm");
    case "client":
      return entry.project?.client.name ?? "";
    case "project":
      return entry.project?.name ?? "";
    case "title":
      return entry.title?.trim() || "（タイトルなし）";
    case "note":
      return entry.note ?? "";
    case "tags":
      return entry.tags.map((tag) => tag.name).join(" / ");
    case "duration":
      return formatReportDuration(
        entry.minutes,
        column.durationFormat ?? "hours-minutes",
      );
    case "durationMinutes":
      return String(entry.minutes);
    case "percentage":
      return totalMinutes > 0
        ? `${Math.round((entry.minutes / totalMinutes) * 100)}%`
        : "0%";
    default:
      return "";
  }
}

export function aggregationColumnValue(
  row: AiAggregationRow,
  column: ReportCopyColumn,
  totalMinutes: number,
): string {
  if (column.kind === "blank") return "";
  if (column.kind === "ai") return row.generatedValues[column.id] ?? "";
  switch (column.field) {
    case "category":
      return row.label;
    case "summary":
      return row.summary;
    case "duration":
      return formatReportDuration(
        row.minutes,
        column.durationFormat ?? "hours-minutes",
      );
    case "durationMinutes":
      return String(row.minutes);
    case "percentage":
      return totalMinutes > 0
        ? `${Math.round((row.minutes / totalMinutes) * 100)}%`
        : "0%";
    case "entryCount":
      return String(row.entryCount);
    default:
      return "";
  }
}

function escapeCell(value: string, delimiter: string): string {
  if (
    value.includes(delimiter) ||
    value.includes("\n") ||
    value.includes("\r") ||
    value.includes('"')
  ) {
    return `"${value.replaceAll('"', '""')}"`;
  }
  return value;
}

function serializeRows(rows: string[][], copyFormat: ReportCopyFormat): string {
  const delimiter = copyFormat.delimiter === "comma" ? "," : "\t";
  const lines = copyFormat.includeHeader
    ? [copyFormat.columns.map(reportCopyColumnHeader), ...rows]
    : rows;
  return lines
    .map((line) => line.map((cell) => escapeCell(cell, delimiter)).join(delimiter))
    .join("\n");
}

export function buildReportCopyText(
  entries: ReportEntry[],
  totalMinutes: number,
  copyFormat: ReportCopyFormat,
): string {
  return serializeRows(
    entries.map((entry) =>
      copyFormat.columns.map((column) =>
        entryColumnValue(entry, column, totalMinutes),
      ),
    ),
    copyFormat,
  );
}

export function buildAggregationCopyText(
  rows: AiAggregationRow[],
  totalMinutes: number,
  copyFormat: ReportCopyFormat,
): string {
  return serializeRows(
    rows.map((row) =>
      copyFormat.columns.map((column) =>
        aggregationColumnValue(row, column, totalMinutes),
      ),
    ),
    copyFormat,
  );
}
