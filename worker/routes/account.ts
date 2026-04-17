import { Hono } from "hono";
import bcrypt from "bcryptjs";
import { getPrisma } from "../db";
import {
  settingsUpdateSchema,
  profileUpdateSchema,
  passwordChangeSchema,
} from "@/lib/validators";
import type { Env, AuthVars } from "../types";

const account = new Hono<{ Bindings: Env; Variables: AuthVars }>();

// GET /api/account — profile + settings
account.get("/", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);
  const user = await prisma.user.findUnique({
    where: { id: userId },
    select: { id: true, email: true, name: true, workStart: true, workEnd: true, workDays: true },
  });
  if (!user) return c.json({ error: "not_found" }, 404);
  return c.json({
    ...user,
    workDays: user.workDays.split(",").map(Number).filter((n) => !isNaN(n)),
  });
});

// PATCH /api/account — update name/email
account.patch("/", async (c) => {
  const userId = c.get("userId");
  const body = await c.req.json().catch(() => null);
  const parsed = profileUpdateSchema.safeParse(body);
  if (!parsed.success) return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);

  const prisma = getPrisma(c.env.DB);

  // Check email uniqueness if changing
  if (parsed.data.email) {
    const existing = await prisma.user.findUnique({ where: { email: parsed.data.email } });
    if (existing && existing.id !== userId) {
      return c.json({ error: "email_taken" }, 409);
    }
  }

  const updated = await prisma.user.update({
    where: { id: userId },
    data: parsed.data,
    select: { id: true, email: true, name: true, workStart: true, workEnd: true, workDays: true },
  });
  return c.json({
    ...updated,
    workDays: updated.workDays.split(",").map(Number).filter((n) => !isNaN(n)),
  });
});

// PATCH /api/account/settings — update work schedule
account.patch("/settings", async (c) => {
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
  return c.json({
    ...updated,
    workDays: updated.workDays.split(",").map(Number).filter((n) => !isNaN(n)),
  });
});

// POST /api/account/password — change password
account.post("/password", async (c) => {
  const userId = c.get("userId");
  const body = await c.req.json().catch(() => null);
  const parsed = passwordChangeSchema.safeParse(body);
  if (!parsed.success) return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);

  const prisma = getPrisma(c.env.DB);
  const user = await prisma.user.findUnique({ where: { id: userId } });
  if (!user) return c.json({ error: "not_found" }, 404);

  const ok = await bcrypt.compare(parsed.data.currentPassword, user.passwordHash);
  if (!ok) return c.json({ error: "wrong_password" }, 401);

  const newHash = await bcrypt.hash(parsed.data.newPassword, 10);
  await prisma.user.update({ where: { id: userId }, data: { passwordHash: newHash } });
  return c.json({ ok: true });
});

export { account };
