import { Hono } from "hono";
import { getPrisma } from "../db";
import { settingsUpdateSchema } from "@/lib/validators";
import type { Env, AuthVars } from "../types";

const settings = new Hono<{ Bindings: Env; Variables: AuthVars }>();

function toSettings(u: { workStart: number; workEnd: number; workDays: string }) {
  return {
    workStart: u.workStart,
    workEnd: u.workEnd,
    workDays: u.workDays.split(",").map(Number).filter((n) => !isNaN(n)),
  };
}

// GET /api/settings — 勤務設定
settings.get("/", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);
  const user = await prisma.user.findUnique({
    where: { id: userId },
    select: { workStart: true, workEnd: true, workDays: true },
  });
  if (!user) return c.json({ error: "not_found" }, 404);
  return c.json(toSettings(user));
});

// PATCH /api/settings — 勤務設定の更新
settings.patch("/", async (c) => {
  const userId = c.get("userId");
  const body = await c.req.json().catch(() => null);
  const parsed = settingsUpdateSchema.safeParse(body);
  if (!parsed.success) return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);

  const data: Record<string, unknown> = {};
  if (parsed.data.workStart !== undefined) data.workStart = parsed.data.workStart;
  if (parsed.data.workEnd !== undefined) data.workEnd = parsed.data.workEnd;
  if (parsed.data.workDays !== undefined) data.workDays = parsed.data.workDays.join(",");

  const prisma = getPrisma(c.env.DB);
  const updated = await prisma.user.update({
    where: { id: userId },
    data,
    select: { workStart: true, workEnd: true, workDays: true },
  });
  return c.json(toSettings(updated));
});

export { settings };
