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
    include: { project: { include: { client: true } } },
  });

  const map = new Map<string, { label: string; color?: string; totalMinutes: number }>();
  let totalMinutes = 0;

  for (const entry of allEntries) {
    const clampedStart = entry.start < from ? from : entry.start;
    const clampedEnd = entry.end > to ? to : entry.end;
    const mins = Math.round((clampedEnd.getTime() - clampedStart.getTime()) / 60_000);

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
