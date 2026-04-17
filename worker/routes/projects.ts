import { Hono } from "hono";
import { getPrisma } from "../db";
import { projectCreateSchema, projectUpdateSchema } from "@/lib/validators";
import type { Env, AuthVars } from "../types";

const projects = new Hono<{ Bindings: Env; Variables: AuthVars }>();

projects.get("/", async (c) => {
  const userId = c.get("userId");
  const clientId = c.req.query("clientId");
  const includeArchived = c.req.query("includeArchived") === "1";
  const prisma = getPrisma(c.env.DB);

  const where: Record<string, unknown> = { userId };
  if (clientId) where.clientId = clientId;
  if (!includeArchived) where.archived = false;

  const list = await prisma.project.findMany({
    where,
    include: { client: true },
    orderBy: { createdAt: "asc" },
  });
  return c.json(list);
});

projects.post("/", async (c) => {
  const userId = c.get("userId");
  const body = await c.req.json().catch(() => null);
  const parsed = projectCreateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const created = await prisma.project.create({
    data: { ...parsed.data, userId },
    include: { client: true },
  });
  return c.json(created, 201);
});

projects.patch("/:id", async (c) => {
  const userId = c.get("userId");
  const id = c.req.param("id");
  const body = await c.req.json().catch(() => null);
  const parsed = projectUpdateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.project.findFirst({ where: { id, userId } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  const updated = await prisma.project.update({
    where: { id },
    data: parsed.data,
    include: { client: true },
  });
  return c.json(updated);
});

projects.delete("/:id", async (c) => {
  const userId = c.get("userId");
  const id = c.req.param("id");
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.project.findFirst({ where: { id, userId } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  const entryCount = await prisma.timeEntry.count({ where: { projectId: id } });
  if (entryCount > 0) {
    return c.json({ error: "project_has_entries" }, 409);
  }
  await prisma.project.delete({ where: { id } });
  return c.json({ ok: true });
});

export { projects };
