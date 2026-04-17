import { Hono } from "hono";
import { getPrisma } from "../db";
import { clientCreateSchema, clientUpdateSchema } from "@/lib/validators";
import type { Env, AuthVars } from "../types";

const clients = new Hono<{ Bindings: Env; Variables: AuthVars }>();

clients.get("/", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);
  const list = await prisma.client.findMany({
    where: { userId },
    orderBy: { createdAt: "asc" },
  });
  return c.json(list);
});

clients.post("/", async (c) => {
  const userId = c.get("userId");
  const body = await c.req.json().catch(() => null);
  const parsed = clientCreateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const created = await prisma.client.create({
    data: { ...parsed.data, userId },
  });
  return c.json(created, 201);
});

clients.patch("/:id", async (c) => {
  const userId = c.get("userId");
  const id = c.req.param("id");
  const body = await c.req.json().catch(() => null);
  const parsed = clientUpdateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.client.findFirst({ where: { id, userId } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  const updated = await prisma.client.update({
    where: { id },
    data: parsed.data,
  });
  return c.json(updated);
});

clients.delete("/:id", async (c) => {
  const userId = c.get("userId");
  const id = c.req.param("id");
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.client.findFirst({ where: { id, userId } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  await prisma.client.delete({ where: { id } });
  return c.json({ ok: true });
});

export { clients };
