import type { ReportCopyFormat, ReportEntriesResponse } from "@shared/types";
import { reportCopyColumnHeader } from "./reportCopy";

export type AiAggregationRow = {
  label: string;
  summary: string;
  minutes: number;
  entryCount: number;
  generatedValues: Record<string, string>;
};

export function buildReportAggregationPrompt(
  data: ReportEntriesResponse,
  copyFormat: ReportCopyFormat,
): string {
  const entries = data.entries.map((entry) => ({
    id: entry.id,
    client: entry.project?.client.name ?? "",
    project: entry.project?.name ?? "",
    title: entry.title ?? "",
    note: entry.note ?? "",
    tags: entry.tags.map((tag) => tag.name),
  }));
  const outputColumns = copyFormat.columns
    .filter((column) => column.kind !== "blank")
    .map((column, index) => ({
      order: index + 1,
      id: column.id,
      label: reportCopyColumnHeader(column),
      source:
        column.kind === "ai"
          ? "AI生成"
          : ["category", "summary"].includes(column.field)
            ? "AI集計結果"
            : "Trackが元データから計算",
      instruction: column.kind === "ai" ? column.prompt : undefined,
    }));
  return `
<集計の観点>
${copyFormat.aiPrompt.trim()}
</集計の観点>

<出力フォーマット>
形式名: ${copyFormat.name}
区切り文字: ${copyFormat.delimiter === "comma" ? "カンマ" : "タブ"}
列（指定順）: ${JSON.stringify(outputColumns)}
AI生成列は各グループのvalues配列に、{"id":"列ID","value":"生成結果"}として入れてください。
</出力フォーマット>

<工数記録>
${JSON.stringify(entries)}
</工数記録>
`.trim();
}

function extractJson(text: string): unknown {
  const trimmed = text.trim().replace(/^```(?:json)?\s*/i, "").replace(/\s*```$/, "");
  const first = trimmed.indexOf("{");
  const last = trimmed.lastIndexOf("}");
  if (first === -1 || last <= first) throw new Error("AIの分類結果を読み取れませんでした。");
  return JSON.parse(trimmed.slice(first, last + 1));
}

export function aggregateAiResult(
  response: string,
  data: ReportEntriesResponse,
): AiAggregationRow[] {
  const parsed = extractJson(response) as {
    groups?: {
      label?: unknown;
      summary?: unknown;
      entryIds?: unknown;
      values?: unknown;
    }[];
  };
  if (!Array.isArray(parsed.groups)) {
    throw new Error("AIの分類結果にグループがありませんでした。");
  }

  const entriesById = new Map(data.entries.map((entry) => [entry.id, entry]));
  const assigned = new Set<string>();
  const rows: AiAggregationRow[] = [];

  for (const group of parsed.groups) {
    if (
      typeof group?.label !== "string" ||
      !group.label.trim() ||
      !Array.isArray(group.entryIds)
    ) {
      continue;
    }
    const groupIds = new Set<string>();
    const ids = group.entryIds.filter((id): id is string => {
      if (
        typeof id !== "string" ||
        !entriesById.has(id) ||
        assigned.has(id) ||
        groupIds.has(id)
      ) {
        return false;
      }
      groupIds.add(id);
      return true;
    });
    if (ids.length === 0) continue;
    ids.forEach((id) => assigned.add(id));
    rows.push({
      label: group.label.trim(),
      summary: typeof group.summary === "string" ? group.summary.trim() : "",
      minutes: ids.reduce((sum, id) => sum + entriesById.get(id)!.minutes, 0),
      entryCount: ids.length,
      generatedValues:
        Array.isArray(group.values)
          ? Object.fromEntries(
              group.values
                .filter(
                  (value): value is { id: string; value: string } =>
                    Boolean(value) &&
                    typeof value === "object" &&
                    typeof (value as { id?: unknown }).id === "string" &&
                    typeof (value as { value?: unknown }).value === "string",
                )
                .map((value) => [value.id, value.value.trim()]),
            )
          : group.values && typeof group.values === "object"
          ? Object.fromEntries(
              Object.entries(group.values as Record<string, unknown>)
                .filter((entry): entry is [string, string] => typeof entry[1] === "string")
                .map(([key, value]) => [key, value.trim()]),
            )
          : {},
    });
  }

  const unassigned = data.entries.filter((entry) => !assigned.has(entry.id));
  if (unassigned.length > 0) {
    rows.push({
      label: "その他",
      summary: "AIが分類しなかった工数記録",
      minutes: unassigned.reduce((sum, entry) => sum + entry.minutes, 0),
      entryCount: unassigned.length,
      generatedValues: {},
    });
  }
  if (rows.length === 0) throw new Error("AIの分類結果が空でした。");
  return rows.sort((a, b) => b.minutes - a.minutes || a.label.localeCompare(b.label, "ja"));
}
