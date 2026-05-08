import { Hono } from "hono";
import { z } from "zod";
import { getPrisma } from "../db";
import { generateToken, hashToken } from "../lib/pat";
import {
  settingsUpdateSchema,
  profileUpdateSchema,
} from "@/lib/validators";
import type { Env, AuthVars } from "../types";

const tokenCreateSchema = z.object({
  name: z.string().trim().min(1).max(100),
});

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

// GET /api/account/tokens — list PATs (no plain text)
account.get("/tokens", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);
  const tokens = await prisma.personalAccessToken.findMany({
    where: { userId },
    select: { id: true, name: true, createdAt: true, lastUsedAt: true },
    orderBy: { createdAt: "desc" },
  });
  return c.json(tokens);
});

// POST /api/account/tokens — issue a new PAT (plain text returned ONCE)
account.post("/tokens", async (c) => {
  const userId = c.get("userId");
  const body = await c.req.json().catch(() => null);
  const parsed = tokenCreateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const plain = generateToken();
  const tokenHash = await hashToken(plain);
  const prisma = getPrisma(c.env.DB);
  const created = await prisma.personalAccessToken.create({
    data: { userId, name: parsed.data.name, tokenHash },
    select: { id: true, name: true, createdAt: true },
  });
  return c.json({ ...created, token: plain }, 201);
});

// DELETE /api/account/tokens/:id — revoke a PAT
account.delete("/tokens/:id", async (c) => {
  const userId = c.get("userId");
  const id = c.req.param("id");
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.personalAccessToken.findFirst({
    where: { id, userId },
  });
  if (!existing) return c.json({ error: "not_found" }, 404);
  await prisma.personalAccessToken.delete({ where: { id } });
  return c.json({ ok: true });
});

export { account };
