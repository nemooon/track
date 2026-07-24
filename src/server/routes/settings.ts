import { Hono } from "hono";
import { getPrisma } from "../db/prisma";
import { settingsUpdateSchema } from "@shared/validators";
import type { Env } from "../types";

const settings = new Hono<{ Bindings: Env }>();

function toSettings(u: {
  workStart: number;
  workEnd: number;
  workDays: string;
  weeklyReportTemplate: string;
}) {
  return {
    workStart: u.workStart,
    workEnd: u.workEnd,
    workDays: u.workDays.split(",").map(Number).filter((n) => !isNaN(n)),
    weeklyReportTemplate: u.weeklyReportTemplate,
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

  // 1行しかないので id を知らずに更新できる
  await prisma.settings.updateMany({ data });

  const row = await prisma.settings.findFirst({
    select: {
      workStart: true,
      workEnd: true,
      workDays: true,
      weeklyReportTemplate: true,
    },
  });
  if (!row) return c.json({ error: "not_found" }, 404);
  return c.json(toSettings(row));
});

export { settings };
