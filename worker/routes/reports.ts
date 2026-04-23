import { Hono } from "hono";
import { getPrisma } from "../db";
import { reportsQuerySchema } from "@/lib/validators";
import { getWeekRange, getMonthRange } from "@/lib/time";
import type { Env, AuthVars } from "../types";

const reports = new Hono<{ Bindings: Env; Variables: AuthVars }>();

reports.get("/", async (c) => {
  const userId = c.get("userId");
  const q = reportsQuerySchema.safeParse({
    range: c.req.query("range"),
    anchor: c.req.query("anchor"),
    groupBy: c.req.query("groupBy"),
  });
  if (!q.success) return c.json({ error: "invalid_input", issues: q.error.flatten() }, 400);

  const { range, anchor, groupBy } = q.data;
  const anchorDate = new Date(anchor);
  const { from, to } = range === "week" ? getWeekRange(anchorDate) : getMonthRange(anchorDate);

  const prisma = getPrisma(c.env.DB);
  const allEntries = await prisma.timeEntry.findMany({
    where: {
      userId,
      start: { lt: to },
      end: { gt: from },
    },
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
    const mins = Math.round((clampedEnd.getTime() - clampedStart.getTime()) / 60_000);

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

      if (tagMap.size === 0) {
        const existing = map.get("__none__");
        if (existing) {
          existing.totalMinutes += mins;
        } else {
          map.set("__none__", { label: "タグなし", color: "#9ca3af", totalMinutes: mins });
        }
      } else {
        for (const [tagId, tag] of tagMap) {
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

export { reports };
