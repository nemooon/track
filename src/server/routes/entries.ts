import { createHash } from "node:crypto";
import { Hono } from "hono";
import { getPrisma } from "../db/prisma";
import { entryCreateSchema, entryUpdateSchema } from "@shared/validators";
import { snapToQuarter, sameDay } from "@shared/time";
import { publishEntryChange } from "../changeEvents";
import type { Env } from "../types";

const entries = new Hono<{ Bindings: Env }>();

entries.get("/", async (c) => {
  const from = c.req.query("from");
  const to = c.req.query("to");
  if (!from || !to) return c.json({ error: "from and to required" }, 400);

  const prisma = getPrisma(c.env.DB);
  const list = await prisma.timeEntry.findMany({
    where: {
      start: { lt: new Date(to) },
      end: { gt: new Date(from) },
    },
    include: { project: { include: { client: true } }, tags: { include: { tag: true } } },
    orderBy: { start: "asc" },
  });
  return c.json(list);
});

entries.get("/ai-progress", async (c) => {
  const source = c.req.query("source");
  const sessionId = c.req.query("sessionId");
  if ((source !== "codex" && source !== "claude") || !sessionId || sessionId.length > 200) {
    return c.json({ error: "invalid_input" }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  if (c.req.query("coverage") === "1") {
    const registrations = await prisma.aiRegistration.findMany({
      where: { source, sessionId }, orderBy: { recordedUntil: "asc" },
      select: { recordedFrom: true, recordedUntil: true },
    });
    return c.json({ registrations });
  }
  const progress = await prisma.aiRegistration.findFirst({
    where: { source, sessionId }, orderBy: { recordedUntil: "desc" },
    select: { recordedUntil: true },
  });
  return c.json({ recordedUntil: progress?.recordedUntil ?? null });
});

entries.get("/titles", async (c) => {
  const projectId = c.req.query("projectId");
  const prisma = getPrisma(c.env.DB);
  const projectFilter =
    projectId === "none"
      ? { projectId: null }
      : projectId
        ? { projectId }
        : {};
  const list = await prisma.timeEntry.findMany({
    where: { title: { not: null }, ...projectFilter },
    select: { title: true },
    orderBy: { start: "desc" },
    take: 500,
  });
  const seen = new Set<string>();
  const titles: string[] = [];
  for (const e of list) {
    const t = e.title?.trim();
    if (!t || seen.has(t)) continue;
    seen.add(t);
    titles.push(t);
    if (titles.length >= 100) break;
  }
  return c.json(titles);
});

entries.post("/", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = entryCreateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  if (parsed.data.aiRegistration &&
    (Date.parse(parsed.data.start) % (15 * 60_000) !== 0 || Date.parse(parsed.data.end) % (15 * 60_000) !== 0)) {
    return c.json({ error: "invalid_time_step" }, 400);
  }
  const start = snapToQuarter(new Date(parsed.data.start));
  const end = snapToQuarter(new Date(parsed.data.end));
  if (!sameDay(start, end) && !(end.getHours() === 0 && end.getMinutes() === 0)) {
    return c.json({ error: "entry_must_be_same_day" }, 400);
  }
  if (end <= start) return c.json({ error: "invalid_rounded_interval" }, 400);
  const prisma = getPrisma(c.env.DB);
  const tagIds = [...new Set(parsed.data.tagIds ?? [])].sort();
  const registration = parsed.data.aiRegistration;
  const entryData = {
    projectId: parsed.data.projectId ?? null,
    start,
    end,
    title: parsed.data.title,
    note: parsed.data.note,
    externalEventId: parsed.data.externalEventId,
    externalEventSource: parsed.data.externalEventSource,
    breakMinutes: parsed.data.breakMinutes ?? 0,
    tags: tagIds.length > 0 ? { create: tagIds.map((tagId) => ({ tagId })) } : undefined,
  };
  const include = { project: { include: { client: true } }, tags: { include: { tag: true } } };
  if (!registration) {
    const created = await prisma.timeEntry.create({ data: entryData, include });
    publishEntryChange({ action: "created", entryId: created.id });
    return c.json(created, 201);
  }
  const payloadHash = createHash("sha256").update(JSON.stringify({
    ...entryData, title: entryData.title ?? null, note: entryData.note ?? null,
    externalEventId: entryData.externalEventId ?? null,
    externalEventSource: entryData.externalEventSource ?? null,
    source: registration.source, sessionId: registration.sessionId,
  })).digest("hex");
  const replay = async () => {
    const prior = await prisma.aiRegistration.findUnique({
      where: { requestId: registration.requestId }, include: { entry: { include } },
    });
    if (!prior) return null;
    if (prior.payloadHash !== payloadHash) return c.json({ error: "request_id_conflict" }, 409);
    if (!prior.entry) return c.json({ error: "registered_entry_deleted" }, 409);
    return c.json(prior.entry, 200);
  };
  const prior = await replay();
  if (prior) return prior;
  try {
    const result = await prisma.$transaction(async (tx) => {
      const overlaps = await tx.timeEntry.findMany({
        where: { start: { lt: end }, end: { gt: start } }, include,
      });
      if (overlaps.some((e) => e.start.getTime() === start.getTime() &&
        e.end.getTime() === end.getTime() && e.title === (entryData.title ?? null))) {
        return { error: "exact_duplicate", overlaps } as const;
      }
      if (overlaps.length && !registration.allowOverlap) {
        return { error: "overlap_requires_confirmation", overlaps } as const;
      }
      const created = await tx.timeEntry.create({ data: entryData, include });
      await tx.aiRegistration.create({ data: {
        requestId: registration.requestId, payloadHash,
        source: registration.source, sessionId: registration.sessionId,
        recordedFrom: start, recordedUntil: end, entryId: created.id,
      } });
      return { created };
    });
    if ("error" in result) {
      // 別要求の重複と、直前に完了した同じ要求の再送を区別する。
      const recovered = await replay();
      if (recovered) return recovered;
      return c.json({ error: result.error, overlaps: result.overlaps }, 409);
    }
    publishEntryChange({ action: "created", entryId: result.created.id });
    return c.json(result.created, 201);
  } catch (error) {
    // 同時実行で要求IDが競合した場合も、成功済みの結果を返す。
    const recovered = await replay();
    if (recovered) return recovered;
    throw error;
  }
});

entries.patch("/:id", async (c) => {
  const id = c.req.param("id");
  const body = await c.req.json().catch(() => null);
  const parsed = entryUpdateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.timeEntry.findUnique({ where: { id } });
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
  publishEntryChange({ action: "updated", entryId: updated.id });
  return c.json(updated);
});

entries.delete("/:id", async (c) => {
  const id = c.req.param("id");
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.timeEntry.findUnique({ where: { id } });
  if (!existing) return c.json({ error: "not_found" }, 404);
  await prisma.timeEntry.delete({ where: { id } });
  publishEntryChange({ action: "deleted", entryId: id });
  return c.json({ ok: true });
});

export { entries };
