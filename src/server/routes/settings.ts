import { Hono } from "hono";
import { getPrisma } from "../db/prisma";
import { settingsUpdateSchema } from "@shared/validators";
import type { Env } from "../types";
import type {
  ReportCopyFormat,
  ReportDurationFormat,
} from "@shared/types";

const settings = new Hono<{ Bindings: Env }>();

const DEFAULT_REPORT_COPY_FORMATS: ReportCopyFormat[] = [
  {
    id: "standard-output",
    name: "標準",
    target: "ai-aggregation",
    delimiter: "tab",
    includeHeader: true,
    aiPrompt: "",
    columns: [
      { id: "standard-output-1", kind: "field", field: "category" },
      {
        id: "standard-output-2",
        kind: "field",
        field: "duration",
        durationFormat: "hours-minutes",
      },
      { id: "standard-output-3", kind: "field", field: "percentage" },
    ],
  },
];

export function parseReportCopyFormats(value: string): ReportCopyFormat[] {
  try {
    const parsed = JSON.parse(value) as unknown;
    const validated = settingsUpdateSchema.shape.reportCopyFormats.safeParse(
      upgradeReportCopyFormats(parsed),
    );
    if (validated.success && validated.data) {
      const aggregationFormats = validated.data.filter(
        (copyFormat) => copyFormat.target === "ai-aggregation",
      );
      return aggregationFormats.length > 0
        ? aggregationFormats
        : DEFAULT_REPORT_COPY_FORMATS;
    }
    return normalizeLegacyFormats(parsed);
  } catch {
    return DEFAULT_REPORT_COPY_FORMATS;
  }
}

function upgradeReportCopyFormats(value: unknown): unknown {
  if (!Array.isArray(value)) return value;
  return value.map((item) => {
    if (!item || typeof item !== "object") return item;
    const raw = item as Record<string, unknown>;
    if (!Array.isArray(raw.columns)) {
      return item;
    }
    const legacyDurationFormat = isReportDurationFormat(raw.durationFormat)
      ? raw.durationFormat
      : "hours-minutes";
    const needsAi = raw.columns.some((column) => {
      if (!column || typeof column !== "object") return false;
      const candidate = column as Record<string, unknown>;
      return candidate.kind === "ai" || candidate.field === "summary";
    });
    return {
      ...raw,
      columns: raw.columns.map((column) => {
        if (!column || typeof column !== "object") return column;
        const candidate = column as Record<string, unknown>;
        if (candidate.kind !== "field" || candidate.field !== "duration") {
          return column;
        }
        return {
          ...candidate,
          durationFormat: isReportDurationFormat(candidate.durationFormat)
            ? candidate.durationFormat
            : legacyDurationFormat,
        };
      }),
      aiPrompt:
        typeof raw.aiPrompt === "string"
          ? raw.aiPrompt
          : needsAi
            ? "作業の目的と内容が同じものをまとめ、報告に使いやすい集計名を付ける"
            : "",
    };
  });
}

function isReportDurationFormat(value: unknown): value is ReportDurationFormat {
  return [
    "hours-minutes",
    "japanese",
    "decimal-with-unit",
    "decimal",
  ].includes(value as ReportDurationFormat);
}

function normalizeLegacyFormats(value: unknown): ReportCopyFormat[] {
  void value;
  return DEFAULT_REPORT_COPY_FORMATS;
}

function toSettings(u: {
  workStart: number;
  workEnd: number;
  workDays: string;
  weeklyReportTemplate: string;
  reportCopyFormats: string;
}) {
  return {
    workStart: u.workStart,
    workEnd: u.workEnd,
    workDays: u.workDays.split(",").map(Number).filter((n) => !isNaN(n)),
    weeklyReportTemplate: u.weeklyReportTemplate,
    reportCopyFormats: parseReportCopyFormats(u.reportCopyFormats),
  };
}

// GET /api/settings — 勤務設定 (Settings は常に1行)
settings.get("/", async (c) => {
  const prisma = getPrisma(c.env.DB);
  const row = await prisma.settings.findFirst({
    select: {
      workStart: true,
      workEnd: true,
      workDays: true,
      weeklyReportTemplate: true,
      reportCopyFormats: true,
    },
  });
  if (!row) return c.json({ error: "not_found" }, 404);
  return c.json(toSettings(row));
});

// PATCH /api/settings — 勤務設定の更新
settings.patch("/", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = settingsUpdateSchema.safeParse(body);
  if (!parsed.success) return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);

  const prisma = getPrisma(c.env.DB);
  const current = await prisma.settings.findFirst({
    select: {
      workStart: true,
      workEnd: true,
      workDays: true,
      weeklyReportTemplate: true,
      reportCopyFormats: true,
    },
  });
  if (!current) return c.json({ error: "not_found" }, 404);

  const nextStart = parsed.data.workStart ?? current.workStart;
  const nextEnd = parsed.data.workEnd ?? current.workEnd;
  const nextDays =
    parsed.data.workDays ??
    current.workDays.split(",").map(Number).filter((n) => !isNaN(n));
  if (nextEnd <= nextStart || nextDays.length === 0) {
    return c.json({ error: "invalid_schedule" }, 400);
  }

  const data: Record<string, unknown> = {};
  if (parsed.data.workStart !== undefined) data.workStart = parsed.data.workStart;
  if (parsed.data.workEnd !== undefined) data.workEnd = parsed.data.workEnd;
  if (parsed.data.workDays !== undefined) data.workDays = parsed.data.workDays.join(",");
  if (parsed.data.weeklyReportTemplate !== undefined) {
    data.weeklyReportTemplate = parsed.data.weeklyReportTemplate;
  }
  if (parsed.data.reportCopyFormats !== undefined) {
    data.reportCopyFormats = JSON.stringify(parsed.data.reportCopyFormats);
  }

  // 1行しかないので id を知らずに更新できる
  await prisma.settings.updateMany({ data });

  const row = await prisma.settings.findFirst({
    select: {
      workStart: true,
      workEnd: true,
      workDays: true,
      weeklyReportTemplate: true,
      reportCopyFormats: true,
    },
  });
  if (!row) return c.json({ error: "not_found" }, 404);
  return c.json(toSettings(row));
});

export { settings };
