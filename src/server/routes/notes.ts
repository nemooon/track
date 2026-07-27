import { Hono } from "hono";
import { getPrisma } from "../db/prisma";
import { noteCreateSchema, noteUpdateSchema } from "@shared/validators";
import type { Env } from "../types";

const notes = new Hono<{ Bindings: Env }>();

const noteInclude = {
  project: { include: { client: true } },
} as const;

notes.get("/", async (c) => {
  const prisma = getPrisma(c.env.DB);
  const query = c.req.query("q")?.trim();
  const projectId = c.req.query("projectId");

  const list = await prisma.note.findMany({
    where: {
      ...(query
        ? {
            OR: [
              { title: { contains: query } },
              { content: { contains: query } },
            ],
          }
        : {}),
      ...(projectId ? { projectId } : {}),
    },
    include: noteInclude,
    orderBy: [{ updatedAt: "desc" }, { createdAt: "desc" }],
  });

  return c.json(list);
});

notes.get("/:id", async (c) => {
  const prisma = getPrisma(c.env.DB);
  const note = await prisma.note.findUnique({
    where: { id: c.req.param("id") },
    include: noteInclude,
  });
  if (!note) return c.json({ error: "not_found" }, 404);
  return c.json(note);
});

notes.post("/", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = noteCreateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }

  const prisma = getPrisma(c.env.DB);
  const created = await prisma.note.create({
    data: parsed.data,
    include: noteInclude,
  });
  return c.json(created, 201);
});

notes.patch("/:id", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = noteUpdateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }

  const prisma = getPrisma(c.env.DB);
  const id = c.req.param("id");
  const existing = await prisma.note.findUnique({ where: { id } });
  if (!existing) return c.json({ error: "not_found" }, 404);

  const updated = await prisma.note.update({
    where: { id },
    data: parsed.data,
    include: noteInclude,
  });
  return c.json(updated);
});

notes.delete("/:id", async (c) => {
  const prisma = getPrisma(c.env.DB);
  const id = c.req.param("id");
  const existing = await prisma.note.findUnique({ where: { id } });
  if (!existing) return c.json({ error: "not_found" }, 404);

  await prisma.note.delete({ where: { id } });
  return c.json({ ok: true });
});

export { notes };
