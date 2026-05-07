import { Hono } from "hono";
import { getPrisma } from "../db";
import { entryCreateSchema, entryUpdateSchema } from "@/lib/validators";
import { snapToQuarter, sameDay } from "@/lib/time";
import type { Env, AuthVars } from "../types";

const entries = new Hono<{ Bindings: Env; Variables: AuthVars }>();

entries.get("/", async (c) => {
  const userId = c.get("userId");
  const from = c.req.query("from");
  const to = c.req.query("to");
  if (!from || !to) return c.json({ error: "from and to required" }, 400);

  const prisma = getPrisma(c.env.DB);
  const list = await prisma.timeEntry.findMany({
    where: {
      userId,
      start: { lt: new Date(to) },
      end: { gt: new Date(from) },
    },
    include: { project: { include: { client: true } }, tags: { include: { tag: true } } },
    orderBy: { start: "asc" },
  });
  return c.json(list);
});

entries.post("/", async (c) => {
  const userId = c.get("userId");
  const body = await c.req.json().catch(() => null);
  const parsed = entryCreateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const start = snapToQuarter(new Date(parsed.data.start));
  const end = snapToQuarter(new Date(parsed.data.end));
  if (!sameDay(start, end) && !(end.getHours() === 0 && end.getMinutes() === 0)) {
    return c.json({ error: "entry_must_be_same_day" }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const tagIds = parsed.data.tagIds ?? [];
  const created = await prisma.timeEntry.create({
    data: {
      userId,
      projectId: parsed.data.projectId ?? null,
      start,
      end,
      title: parsed.data.title,
      note: parsed.data.note,
      externalEventId: parsed.data.externalEventId,
      externalEventSource: parsed.data.externalEventSource,
      breakMinutes: parsed.data.breakMinutes ?? 0,
      tags: tagIds.length > 0 ? { create: tagIds.map((tagId) => ({ tagId })) } : undefined,
    },
    include: { project: { include: { client: true } }, tags: { include: { tag: true } } },
  });
  return c.json(created, 201);
});

entries.patch("/:id", async (c) => {
  const userId = c.get("userId");
  const id = c.req.param("id");
  const body = await c.req.json().catch(() => null);
  const parsed = entryUpdateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.timeEntry.findFirst({ where: { id, userId } });
  if (!existing) return c.json({ error: "not_found" }, 404);

  const data: Record<string, unknown> = {};
  if (parsed.data.projectId !== undefined) data.projectId = parsed.data.projectId;
  if (parsed.data.title !== undefined) data.title = parsed.data.title;
  if (parsed.data.note !== undefined) data.note = parsed.data.note;
  if (parsed.data.start) data.start = snapToQuarter(new Date(parsed.data.start));
  if (parsed.data.end) data.end = snapToQuarter(new Date(parsed.data.end));
  if (parsed.data.breakMinutes !== undefined) data.breakMinutes = parsed.data.breakMinutes;

  const newStart = (data.start as Date) ?? existing.start;
  const newEnd = (data.end as Date) ?? existing.end;
  if (!sameDay(newStart, newEnd) && !(newEnd.getHours() === 0 && newEnd.getMinutes() === 0)) {
    return c.json({ error: "entry_must_be_same_day" }, 400);
  }

  if (parsed.data.tagIds !== undefined) {
    data.tags = {
      deleteMany: {},
      create: parsed.data.tagIds.map((tagId: string) => ({ tagId })),
    };
  }

  const updated = await prisma.timeEntry.update({
    where: { id },
    data,
    include: { project: { include: { client: true } }, tags: { include: { tag: true } } },
  });
  return c.json(updated);
});

entries.delete("/:id", async (c) => {
  const userId = c.get("userId");
  const id = c.req.param("id");
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.timeEntry.findFirst({ where: { id, userId } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  await prisma.timeEntry.delete({ where: { id } });
  return c.json({ ok: true });
});

export { entries };
