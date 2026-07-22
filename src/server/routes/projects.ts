import { Hono } from "hono";
import { getPrisma } from "../db/prisma";
import { projectCreateSchema, projectUpdateSchema } from "@shared/validators";
import type { Env } from "../types";

const projects = new Hono<{ Bindings: Env }>();

projects.get("/", async (c) => {
  const clientId = c.req.query("clientId");
  const includeArchived = c.req.query("includeArchived") === "1";
  const prisma = getPrisma(c.env.DB);

  const where: Record<string, unknown> = {};
  if (clientId) where.clientId = clientId;
  if (!includeArchived) where.archived = false;

  const list = await prisma.project.findMany({
    where,
    include: { client: true, tags: { include: { tag: true } } },
    orderBy: { createdAt: "asc" },
  });
  return c.json(list);
});

projects.post("/", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = projectCreateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const { tagIds, ...projectData } = parsed.data;
  const created = await prisma.project.create({
    data: {
      ...projectData,
      tags: tagIds?.length ? { create: tagIds.map((tagId) => ({ tagId })) } : undefined,
    },
    include: { client: true, tags: { include: { tag: true } } },
  });
  return c.json(created, 201);
});

projects.patch("/:id", async (c) => {
  const id = c.req.param("id");
  const body = await c.req.json().catch(() => null);
  const parsed = projectUpdateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.project.findUnique({ where: { id } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  const { tagIds, ...updateData } = parsed.data;
  const data: Record<string, unknown> = { ...updateData };
  if (tagIds !== undefined) {
    data.tags = {
      deleteMany: {},
      create: tagIds.map((tagId: string) => ({ tagId })),
    };
  }
  const updated = await prisma.project.update({
    where: { id },
    data,
    include: { client: true, tags: { include: { tag: true } } },
  });
  return c.json(updated);
});

projects.delete("/:id", async (c) => {
  const id = c.req.param("id");
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.project.findUnique({ where: { id } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  const entryCount = await prisma.timeEntry.count({ where: { projectId: id } });
  if (entryCount > 0) {
    return c.json({ error: "project_has_entries" }, 409);
  }
  await prisma.project.delete({ where: { id } });
  return c.json({ ok: true });
});

export { projects };
