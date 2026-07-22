import { Hono } from "hono";
import { getPrisma } from "../db/prisma";
import { clientCreateSchema, clientUpdateSchema } from "@shared/validators";
import type { Env } from "../types";

const clients = new Hono<{ Bindings: Env }>();

clients.get("/", async (c) => {
  const prisma = getPrisma(c.env.DB);
  const list = await prisma.client.findMany({
    orderBy: { createdAt: "asc" },
  });
  return c.json(list);
});

clients.post("/", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = clientCreateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const created = await prisma.client.create({
    data: parsed.data,
  });
  return c.json(created, 201);
});

clients.patch("/:id", async (c) => {
  const id = c.req.param("id");
  const body = await c.req.json().catch(() => null);
  const parsed = clientUpdateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.client.findUnique({ where: { id } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  const updated = await prisma.client.update({
    where: { id },
    data: parsed.data,
  });
  return c.json(updated);
});

clients.delete("/:id", async (c) => {
  const id = c.req.param("id");
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.client.findUnique({ where: { id } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  await prisma.client.delete({ where: { id } });
  return c.json({ ok: true });
});

export { clients };
