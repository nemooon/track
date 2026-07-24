import { Hono } from "hono";
import { getPrisma } from "../db/prisma";
import { reportsQuerySchema, reportsEntriesQuerySchema } from "@shared/validators";
import { getWeekRange, getMonthRange } from "@shared/time";
import type { Env } from "../types";

const reports = new Hono<{ Bindings: Env }>();

type EntryFilters = {
  clientIds?: string[];
  projectIds?: string[];
  tagIds?: string[];
};

// "__none__" is a sentinel meaning "no value": no project (and thus no client),
// or no tags on the entry / project.
const NONE = "__none__";

function splitNone(ids: string[] | undefined): { real: string[]; hasNone: boolean } {
  if (!ids || ids.length === 0) return { real: [], hasNone: false };
  const real = ids.filter((id) => id !== NONE);
  return { real, hasNone: real.length !== ids.length };
}

function noTagCondition(): Record<string, unknown> {
  // Entry has no tags AND (no project OR project has no tags)
  return {
    tags: { none: {} },
    OR: [{ project: null }, { project: { tags: { none: {} } } }],
  };
}

function buildWhere(from: Date, to: Date, f: EntryFilters) {
  const where: Record<string, unknown> = {
    start: { lt: to },
    end: { gt: from },
  };
  const ands: Record<string, unknown>[] = [];

  const c = splitNone(f.clientIds);
  if (c.hasNone || c.real.length > 0) {
    const conds: Record<string, unknown>[] = [];
    if (c.hasNone) conds.push({ projectId: null });
    if (c.real.length > 0) conds.push({ project: { clientId: { in: c.real } } });
    ands.push(conds.length === 1 ? conds[0] : { OR: conds });
  }

  const p = splitNone(f.projectIds);
  if (p.hasNone || p.real.length > 0) {
    const conds: Record<string, unknown>[] = [];
    if (p.hasNone) conds.push({ projectId: null });
    if (p.real.length > 0) conds.push({ projectId: { in: p.real } });
    ands.push(conds.length === 1 ? conds[0] : { OR: conds });
  }

  const t = splitNone(f.tagIds);
  if (t.hasNone || t.real.length > 0) {
    const conds: Record<string, unknown>[] = [];
    if (t.hasNone) conds.push(noTagCondition());
    if (t.real.length > 0) {
      conds.push({
        OR: [
          { tags: { some: { tagId: { in: t.real } } } },
          { project: { tags: { some: { tagId: { in: t.real } } } } },
        ],
      });
    }
    ands.push(conds.length === 1 ? conds[0] : { OR: conds });
  }

  if (ands.length > 0) where.AND = ands;
  return where;
}

reports.get("/", async (c) => {
  const q = reportsQuerySchema.safeParse({
    range: c.req.query("range"),
    anchor: c.req.query("anchor"),
    groupBy: c.req.query("groupBy"),
    clientIds: c.req.query("clientIds"),
    projectIds: c.req.query("projectIds"),
    tagIds: c.req.query("tagIds"),
  });
  if (!q.success) return c.json({ error: "invalid_input", issues: q.error.flatten() }, 400);

  const { range, anchor, groupBy, clientIds, projectIds, tagIds } = q.data;
  const anchorDate = new Date(anchor);
  const { from, to } = range === "week" ? getWeekRange(anchorDate) : getMonthRange(anchorDate);

  const prisma = getPrisma(c.env.DB);
  const allEntries = await prisma.timeEntry.findMany({
    where: buildWhere(from, to, { clientIds, projectIds, tagIds }),
    include: {
      project: { include: { client: true, tags: { include: { tag: true } } } },
      tags: { include: { tag: true } },
    },
  });

  const map = new Map<string, { label: string; color?: string; totalMinutes: number }>();
  let totalMinutes = 0;

  for (const entry of allEntries) {
    const clampedStart = entry.start < from ? from : entry.start;
    const clampedEnd = entry.end > to ? to : entry.end;
    const grossMins = Math.round((clampedEnd.getTime() - clampedStart.getTime()) / 60_000);
    // Subtract embedded break only when the entry is fully inside the range; otherwise prorate.
    const fullMs = entry.end.getTime() - entry.start.getTime();
    const visMs = clampedEnd.getTime() - clampedStart.getTime();
    const breakShare = fullMs > 0 ? Math.round((entry.breakMinutes ?? 0) * (visMs / fullMs)) : 0;
    const mins = Math.max(0, grossMins - breakShare);

    if (groupBy === "tag") {
      // Merge entry tags + project tags, deduplicated by tagId
      const tagMap = new Map<string, { name: string; color: string }>();
      for (const te of entry.tags) {
        tagMap.set(te.tagId, { name: te.tag.name, color: te.tag.color });
      }
      if (entry.project) {
        for (const tp of (entry.project as unknown as { tags: { tagId: string; tag: { name: string; color: string } }[] }).tags) {
          if (!tagMap.has(tp.tagId)) {
            tagMap.set(tp.tagId, { name: tp.tag.name, color: tp.tag.color });
          }
        }
      }

      // When tagIds filter is active, only count tags that are in the filter
      // (an entry can match via project tag but the entry itself or project may have other tags too)
      const includeAll = !tagIds || tagIds.length === 0;
      const visibleTags = includeAll
        ? Array.from(tagMap.entries())
        : Array.from(tagMap.entries()).filter(([id]) => tagIds.includes(id));

      if (visibleTags.length === 0) {
        if (includeAll) {
          const existing = map.get("__none__");
          if (existing) {
            existing.totalMinutes += mins;
          } else {
            map.set("__none__", { label: "タグなし", color: "#9ca3af", totalMinutes: mins });
          }
        }
      } else {
        for (const [tagId, tag] of visibleTags) {
          const existing = map.get(tagId);
          if (existing) {
            existing.totalMinutes += mins;
          } else {
            map.set(tagId, { label: tag.name, color: tag.color, totalMinutes: mins });
          }
        }
      }
      totalMinutes += mins;
      continue;
    }

    let key: string;
    let label: string;
    let color: string | undefined;

    if (!entry.project) {
      key = "__none__";
      label = "プロジェクトなし";
      color = "#9ca3af";
    } else if (groupBy === "client") {
      key = entry.project.clientId;
      label = entry.project.client.name;
    } else {
      key = entry.projectId!;
      label = `${entry.project.client.name} · ${entry.project.name}`;
      color = entry.project.color;
    }

    const existing = map.get(key);
    if (existing) {
      existing.totalMinutes += mins;
    } else {
      map.set(key, { label, color, totalMinutes: mins });
    }
    totalMinutes += mins;
  }

  const rows = Array.from(map.entries()).map(([key, v]) => ({ key, ...v }));
  rows.sort((a, b) => b.totalMinutes - a.totalMinutes);

  return c.json({ rows, totalMinutes, range: { from: from.toISOString(), to: to.toISOString() } });
});

reports.get("/entries", async (c) => {
  const q = reportsEntriesQuerySchema.safeParse({
    range: c.req.query("range"),
    anchor: c.req.query("anchor"),
    clientIds: c.req.query("clientIds"),
    projectIds: c.req.query("projectIds"),
    tagIds: c.req.query("tagIds"),
  });
  if (!q.success) return c.json({ error: "invalid_input", issues: q.error.flatten() }, 400);

  const { range, anchor, clientIds, projectIds, tagIds } = q.data;
  const anchorDate = new Date(anchor);
  const { from, to } = range === "week" ? getWeekRange(anchorDate) : getMonthRange(anchorDate);

  const prisma = getPrisma(c.env.DB);
  const allEntries = await prisma.timeEntry.findMany({
    where: buildWhere(from, to, { clientIds, projectIds, tagIds }),
    include: {
      project: { include: { client: true } },
      tags: { include: { tag: true } },
    },
    orderBy: { start: "asc" },
  });

  let totalMinutes = 0;
  const entries = allEntries.map((entry) => {
    const clampedStart = entry.start < from ? from : entry.start;
    const clampedEnd = entry.end > to ? to : entry.end;
    const grossMinutes = Math.round((clampedEnd.getTime() - clampedStart.getTime()) / 60_000);
    const fullMs = entry.end.getTime() - entry.start.getTime();
    const visMs = clampedEnd.getTime() - clampedStart.getTime();
    const breakShare = fullMs > 0 ? Math.round((entry.breakMinutes ?? 0) * (visMs / fullMs)) : 0;
    const minutes = Math.max(0, grossMinutes - breakShare);
    totalMinutes += minutes;
    return {
      id: entry.id,
      start: entry.start.toISOString(),
      end: entry.end.toISOString(),
      minutes,
      title: entry.title,
      note: entry.note,
      project: entry.project
        ? {
            id: entry.project.id,
            name: entry.project.name,
            color: entry.project.color,
            client: { id: entry.project.client.id, name: entry.project.client.name },
          }
        : null,
      tags: entry.tags.map((te) => ({
        id: te.tag.id,
        name: te.tag.name,
        color: te.tag.color,
      })),
    };
  });

  return c.json({ entries, totalMinutes });
});

export { reports };
