import { Hono } from "hono";
import { getPrisma } from "../db";
import { tagCreateSchema, tagUpdateSchema } from "@/lib/validators";
import type { Env, AuthVars } from "../types";

const tags = new Hono<{ Bindings: Env; Variables: AuthVars }>();

tags.get("/", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);
  const list = await prisma.tag.findMany({
    where: { userId },
    orderBy: { name: "asc" },
  });
  return c.json(list);
});

tags.post("/", async (c) => {
  const userId = c.get("userId");
  const body = await c.req.json().catch(() => null);
  const parsed = tagCreateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.tag.findUnique({
    where: { userId_name: { userId, name: parsed.data.name } },
  });
  if (existing) {
    return c.json({ error: "tag_already_exists" }, 409);
  }
  const created = await prisma.tag.create({
    data: { userId, name: parsed.data.name, color: parsed.data.color },
  });
  return c.json(created, 201);
});

tags.patch("/:id", async (c) => {
  const userId = c.get("userId");
  const id = c.req.param("id");
  const body = await c.req.json().catch(() => null);
  const parsed = tagUpdateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.tag.findFirst({ where: { id, userId } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  const updated = await prisma.tag.update({
    where: { id },
    data: parsed.data,
  });
  return c.json(updated);
});

tags.delete("/:id", async (c) => {
  const userId = c.get("userId");
  const id = c.req.param("id");
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.tag.findFirst({ where: { id, userId } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  await prisma.tag.delete({ where: { id } });
  return c.json({ ok: true });
});

export { tags };
