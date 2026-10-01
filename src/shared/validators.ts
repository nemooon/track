import { z } from "zod";

export const emailSchema = z.string().email().max(200);
const isoDateTimeSchema = z.string().datetime({ offset: true });

export const signupSchema = z.object({
  email: emailSchema.refine((e) => e.endsWith("@alfasado.jp"), "このメールアドレスでは登録できません"),
  name: z.string().max(100).optional(),
});

export const clientCreateSchema = z.object({
  name: z.string().min(1).max(100),
});

export const clientUpdateSchema = z.object({
  name: z.string().min(1).max(100).optional(),
  archived: z.boolean().optional(),
});

export const projectCreateSchema = z.object({
  clientId: z.string().min(1),
  name: z.string().min(1).max(100),
  color: z.string().regex(/^#[0-9a-fA-F]{6}$/),
  tagIds: z.array(z.string().min(1)).optional(),
});

export const projectUpdateSchema = z.object({
  clientId: z.string().min(1).optional(),
  name: z.string().min(1).max(100).optional(),
  color: z
    .string()
    .regex(/^#[0-9a-fA-F]{6}$/)
    .optional(),
  archived: z.boolean().optional(),
  tagIds: z.array(z.string().min(1)).optional(),
});

export const noteCreateSchema = z.object({
  title: z.string().trim().min(1).max(200),
  content: z.string().max(100_000).optional(),
  projectId: z.string().min(1).nullable().optional(),
});

export const noteUpdateSchema = z.object({
  title: z.string().trim().min(1).max(200).optional(),
  content: z.string().max(100_000).optional(),
  projectId: z.string().min(1).nullable().optional(),
  archived: z.boolean().optional(),
  pinned: z.boolean().optional(),
});

export const entryCreateSchema = z
  .object({
    projectId: z.string().min(1).nullable().optional(),
    start: isoDateTimeSchema,
    end: isoDateTimeSchema,
    title: z.string().max(100).nullable().optional(),
    note: z.string().max(500).nullable().optional(),
    tagIds: z.array(z.string().min(1)).optional(),
    aiRegistration: z.object({
      requestId: z.string().uuid(),
      source: z.enum(["codex", "claude"]),
      sessionId: z.string().min(1).max(200),
      allowOverlap: z.boolean().default(false),
    }).optional(),
    externalEventId: z.string().min(1).max(200).optional(),
    externalEventSource: z.enum(["kot", "outlook"]).optional(),
    breakMinutes: z.number().int().min(0).max(600).optional(),
  })
  .refine((v) => new Date(v.end) > new Date(v.start), "end must be after start");

export const entryUpdateSchema = z
  .object({
    projectId: z.string().min(1).nullable().optional(),
    start: isoDateTimeSchema.optional(),
    end: isoDateTimeSchema.optional(),
    title: z.string().max(100).nullable().optional(),
    note: z.string().max(500).nullable().optional(),
    tagIds: z.array(z.string().min(1)).optional(),
    breakMinutes: z.number().int().min(0).max(600).optional(),
  })
  .refine(
    (v) => !v.start || !v.end || new Date(v.end) > new Date(v.start),
    "end must be after start",
  );

export const entryRangeSchema = z.object({
  from: isoDateTimeSchema,
  to: isoDateTimeSchema,
});

export const tagCreateSchema = z.object({
  name: z.string().min(1).max(50),
  color: z.string().regex(/^#[0-9a-fA-F]{6}$/),
});

export const tagUpdateSchema = z.object({
  name: z.string().min(1).max(50).optional(),
  color: z
    .string()
    .regex(/^#[0-9a-fA-F]{6}$/)
    .optional(),
});

const csvIds = z
  .string()
  .optional()
  .transform((s) => (s ? s.split(",").filter(Boolean) : undefined));

export const reportsQuerySchema = z.object({
  range: z.enum(["week", "month"]),
  anchor: isoDateTimeSchema,
  groupBy: z.enum(["client", "project", "tag"]),
  clientIds: csvIds,
  projectIds: csvIds,
  tagIds: csvIds,
});

export const reportsEntriesQuerySchema = z.object({
  range: z.enum(["week", "month"]),
  anchor: isoDateTimeSchema,
  clientIds: csvIds,
  projectIds: csvIds,
  tagIds: csvIds,
});

const reportCopyFieldSchema = z.enum([
  "date",
  "start",
  "end",
  "client",
  "project",
  "title",
  "note",
  "tags",
  "duration",
  "durationMinutes",
  "percentage",
  "category",
  "summary",
  "entryCount",
]);

const reportCopyColumnSchema = z.discriminatedUnion("kind", [
  z.object({
    id: z.string().min(1).max(100),
    kind: z.literal("field"),
    field: reportCopyFieldSchema,
    label: z.string().max(100).optional(),
    durationFormat: z.enum([
      "hours-minutes",
      "japanese",
      "decimal-with-unit",
      "decimal",
    ]).optional(),
  }),
  z.object({
    id: z.string().min(1).max(100),
    kind: z.literal("blank"),
    label: z.string().max(100).optional(),
  }),
  z.object({
    id: z.string().min(1).max(100),
    kind: z.literal("ai"),
    label: z.string().trim().min(1).max(100),
    prompt: z.string().trim().min(1).max(1_000),
  }),
]);

const entryFields = new Set([
  "date", "start", "end", "client", "project", "title", "note", "tags",
  "duration", "durationMinutes", "percentage",
]);
const aggregationFields = new Set([
  "category", "summary", "duration", "durationMinutes", "percentage", "entryCount",
]);

const reportCopyFormatSchema = z
  .object({
    id: z.string().min(1).max(100),
    name: z.string().trim().min(1).max(100),
    target: z.enum(["entries", "ai-aggregation"]),
    delimiter: z.enum(["tab", "comma"]),
    includeHeader: z.boolean(),
    aiPrompt: z.string().trim().max(10_000),
    columns: z.array(reportCopyColumnSchema).min(1).max(30),
  })
  .superRefine((copyFormat, ctx) => {
    const allowed = copyFormat.target === "entries" ? entryFields : aggregationFields;
    copyFormat.columns.forEach((column, index) => {
      if (column.kind === "ai" && copyFormat.target !== "ai-aggregation") {
        ctx.addIssue({
          code: "custom",
          path: ["columns", index],
          message: "AI生成項目はカスタム出力でのみ利用できます",
        });
      } else if (column.kind === "field" && !allowed.has(column.field)) {
        ctx.addIssue({
          code: "custom",
          path: ["columns", index, "field"],
          message: "対象では利用できない項目です",
        });
      }
    });
  });

export const settingsUpdateSchema = z.object({
  workStart: z.number().int().min(0).max(1440).optional(),
  workEnd: z.number().int().min(0).max(1440).optional(),
  workDays: z.array(z.number().int().min(0).max(6)).optional(),
  weeklyReportTemplate: z.string().trim().min(1).max(10_000).optional(),
  reportCopyFormats: z.array(reportCopyFormatSchema).min(1).max(20).optional(),
});
