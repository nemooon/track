import { Hono } from "hono";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import { toReqRes, toFetchResponse } from "fetch-to-node";
import { z } from "zod";
import { getPrisma } from "./db";
import { reportsQuerySchema } from "@/lib/validators";
import { getWeekRange, getMonthRange, snapToQuarter, sameDay } from "@/lib/time";
import type { Env, AuthVars } from "./types";

type Prisma = ReturnType<typeof getPrisma>;

function asText(data: unknown) {
  return {
    content: [
      {
        type: "text" as const,
        text: typeof data === "string" ? data : JSON.stringify(data, null, 2),
      },
    ],
  };
}

function buildServer(userId: string, prisma: Prisma) {
  const server = new McpServer({ name: "track-mcp", version: "0.2.0" });

  const colorHex = z
    .string()
    .regex(/^#[0-9a-fA-F]{6}$/, "color must be a 6-digit hex like #ff0000");

  server.tool(
    "list_clients",
    "List clients belonging to the authenticated user.",
    {},
    async () => {
      const list = await prisma.client.findMany({
        where: { userId },
        orderBy: { createdAt: "asc" },
      });
      return asText(list);
    },
  );

  server.tool(
    "create_client",
    "Create a new client for the authenticated user.",
    {
      name: z.string().min(1).max(100),
    },
    async ({ name }) => {
      const created = await prisma.client.create({
        data: { userId, name },
      });
      return asText(created);
    },
  );

  server.tool(
    "update_client",
    "Update fields of an existing client. Only provided fields change.",
    {
      id: z.string().min(1),
      name: z.string().min(1).max(100).optional(),
      archived: z.boolean().optional(),
    },
    async ({ id, name, archived }) => {
      const existing = await prisma.client.findFirst({ where: { id, userId } });
      if (!existing) return asText({ error: "not_found" });
      const data: Record<string, unknown> = {};
      if (name !== undefined) data.name = name;
      if (archived !== undefined) data.archived = archived;
      const updated = await prisma.client.update({ where: { id }, data });
      return asText(updated);
    },
  );

  server.tool(
    "delete_client",
    "Delete a client. Fails if the client still has projects.",
    {
      id: z.string().min(1),
    },
    async ({ id }) => {
      const existing = await prisma.client.findFirst({ where: { id, userId } });
      if (!existing) return asText({ error: "not_found" });
      const projectCount = await prisma.project.count({ where: { clientId: id } });
      if (projectCount > 0) {
        return asText({ error: "client_has_projects", projectCount });
      }
      await prisma.client.delete({ where: { id } });
      return asText({ ok: true, id });
    },
  );

  server.tool(
    "list_projects",
    "List Track projects belonging to the authenticated user.",
    {
      archived: z
        .boolean()
        .optional()
        .describe("If true, include archived projects. Default: false."),
    },
    async ({ archived }) => {
      const list = await prisma.project.findMany({
        where: { userId, ...(archived ? {} : { archived: false }) },
        include: { client: true, tags: { include: { tag: true } } },
        orderBy: { createdAt: "asc" },
      });
      return asText(list);
    },
  );

  server.tool(
    "create_project",
    "Create a new project under a client.",
    {
      clientId: z.string().min(1),
      name: z.string().min(1).max(100),
      color: colorHex,
      tagIds: z
        .array(z.string().min(1))
        .optional()
        .describe("Tags to attach as project defaults."),
    },
    async ({ clientId, name, color, tagIds }) => {
      const client = await prisma.client.findFirst({
        where: { id: clientId, userId },
      });
      if (!client) return asText({ error: "client_not_found" });
      const created = await prisma.project.create({
        data: {
          userId,
          clientId,
          name,
          color,
          tags: tagIds?.length
            ? { create: tagIds.map((tagId) => ({ tagId })) }
            : undefined,
        },
        include: { client: true, tags: { include: { tag: true } } },
      });
      return asText(created);
    },
  );

  server.tool(
    "update_project",
    "Update fields of an existing project. Only provided fields change. Pass tagIds to fully replace the project's default tag set.",
    {
      id: z.string().min(1),
      clientId: z.string().min(1).optional(),
      name: z.string().min(1).max(100).optional(),
      color: colorHex.optional(),
      archived: z.boolean().optional(),
      tagIds: z.array(z.string().min(1)).optional(),
    },
    async ({ id, clientId, name, color, archived, tagIds }) => {
      const existing = await prisma.project.findFirst({ where: { id, userId } });
      if (!existing) return asText({ error: "not_found" });
      const data: Record<string, unknown> = {};
      if (clientId !== undefined) data.clientId = clientId;
      if (name !== undefined) data.name = name;
      if (color !== undefined) data.color = color;
      if (archived !== undefined) data.archived = archived;
      if (tagIds !== undefined) {
        data.tags = {
          deleteMany: {},
          create: tagIds.map((tagId) => ({ tagId })),
        };
      }
      const updated = await prisma.project.update({
        where: { id },
        data,
        include: { client: true, tags: { include: { tag: true } } },
      });
      return asText(updated);
    },
  );

  server.tool(
    "delete_project",
    "Delete a project. Fails if it still has time entries.",
    {
      id: z.string().min(1),
    },
    async ({ id }) => {
      const existing = await prisma.project.findFirst({ where: { id, userId } });
      if (!existing) return asText({ error: "not_found" });
      const entryCount = await prisma.timeEntry.count({ where: { projectId: id } });
      if (entryCount > 0) {
        return asText({ error: "project_has_entries", entryCount });
      }
      await prisma.project.delete({ where: { id } });
      return asText({ ok: true, id });
    },
  );

  server.tool(
    "list_tags",
    "List all tags belonging to the authenticated user.",
    {},
    async () => {
      const list = await prisma.tag.findMany({
        where: { userId },
        orderBy: { name: "asc" },
      });
      return asText(list);
    },
  );

  server.tool(
    "create_tag",
    "Create a new tag. Fails if a tag with the same name already exists.",
    {
      name: z.string().min(1).max(50),
      color: colorHex,
    },
    async ({ name, color }) => {
      const existing = await prisma.tag.findUnique({
        where: { userId_name: { userId, name } },
      });
      if (existing) return asText({ error: "tag_already_exists" });
      const created = await prisma.tag.create({
        data: { userId, name, color },
      });
      return asText(created);
    },
  );

  server.tool(
    "update_tag",
    "Update a tag's name or color. Only provided fields change.",
    {
      id: z.string().min(1),
      name: z.string().min(1).max(50).optional(),
      color: colorHex.optional(),
    },
    async ({ id, name, color }) => {
      const existing = await prisma.tag.findFirst({ where: { id, userId } });
      if (!existing) return asText({ error: "not_found" });
      const data: Record<string, unknown> = {};
      if (name !== undefined) data.name = name;
      if (color !== undefined) data.color = color;
      const updated = await prisma.tag.update({ where: { id }, data });
      return asText(updated);
    },
  );

  server.tool(
    "delete_tag",
    "Delete a tag. Removes it from any projects or entries it was attached to.",
    {
      id: z.string().min(1),
    },
    async ({ id }) => {
      const existing = await prisma.tag.findFirst({ where: { id, userId } });
      if (!existing) return asText({ error: "not_found" });
      await prisma.tag.delete({ where: { id } });
      return asText({ ok: true, id });
    },
  );

  server.tool(
    "list_entries",
    "List time entries within a date range. Range is [from, to) where boundaries are ISO 8601 datetimes. Returned entries include project and tag details.",
    {
      from: z
        .string()
        .describe("Inclusive lower bound, ISO 8601 datetime."),
      to: z
        .string()
        .describe("Exclusive upper bound, ISO 8601 datetime."),
    },
    async ({ from, to }) => {
      const list = await prisma.timeEntry.findMany({
        where: {
          userId,
          start: { lt: new Date(to) },
          end: { gt: new Date(from) },
        },
        include: {
          project: { include: { client: true } },
          tags: { include: { tag: true } },
        },
        orderBy: { start: "asc" },
      });
      return asText(list);
    },
  );

  server.tool(
    "create_entry",
    "Create a time entry for the authenticated user. Times are snapped to 15-minute boundaries. Start and end must fall on the same day (end may be 00:00 of the next day).",
    {
      start: z
        .string()
        .describe("Start time, ISO 8601 datetime."),
      end: z
        .string()
        .describe("End time, ISO 8601 datetime. Must be after start."),
      projectId: z
        .string()
        .min(1)
        .nullable()
        .optional()
        .describe("Project ID to associate. Omit or null for no project."),
      title: z.string().max(100).nullable().optional(),
      note: z.string().max(500).nullable().optional(),
      tagIds: z.array(z.string().min(1)).optional(),
      breakMinutes: z.number().int().min(0).max(600).optional(),
    },
    async ({ start, end, projectId, title, note, tagIds, breakMinutes }) => {
      const startDate = snapToQuarter(new Date(start));
      const endDate = snapToQuarter(new Date(end));
      if (endDate <= startDate) {
        return asText({ error: "end_must_be_after_start" });
      }
      if (
        !sameDay(startDate, endDate) &&
        !(endDate.getHours() === 0 && endDate.getMinutes() === 0)
      ) {
        return asText({ error: "entry_must_be_same_day" });
      }
      const ids = tagIds ?? [];
      const created = await prisma.timeEntry.create({
        data: {
          userId,
          projectId: projectId ?? null,
          start: startDate,
          end: endDate,
          title: title ?? null,
          note: note ?? null,
          breakMinutes: breakMinutes ?? 0,
          tags: ids.length > 0 ? { create: ids.map((tagId) => ({ tagId })) } : undefined,
        },
        include: {
          project: { include: { client: true } },
          tags: { include: { tag: true } },
        },
      });
      return asText(created);
    },
  );

  server.tool(
    "update_entry",
    "Update fields of an existing time entry owned by the authenticated user. Only provided fields are changed; omit a field to leave it as-is. Pass null to clear projectId/title/note. Times are snapped to 15-minute boundaries and the resulting range must stay within one day (end may be 00:00 of the next day).",
    {
      id: z.string().min(1).describe("Time entry ID to update."),
      start: z.string().optional().describe("New start, ISO 8601 datetime."),
      end: z.string().optional().describe("New end, ISO 8601 datetime."),
      projectId: z
        .string()
        .min(1)
        .nullable()
        .optional()
        .describe("Project ID, or null to detach from project."),
      title: z.string().max(100).nullable().optional(),
      note: z.string().max(500).nullable().optional(),
      tagIds: z
        .array(z.string().min(1))
        .optional()
        .describe("Full replacement of tag set. Omit to leave tags unchanged."),
      breakMinutes: z.number().int().min(0).max(600).optional(),
    },
    async ({ id, start, end, projectId, title, note, tagIds, breakMinutes }) => {
      const existing = await prisma.timeEntry.findFirst({
        where: { id, userId },
      });
      if (!existing) return asText({ error: "not_found" });

      const data: Record<string, unknown> = {};
      if (projectId !== undefined) data.projectId = projectId;
      if (title !== undefined) data.title = title;
      if (note !== undefined) data.note = note;
      if (start !== undefined) data.start = snapToQuarter(new Date(start));
      if (end !== undefined) data.end = snapToQuarter(new Date(end));
      if (breakMinutes !== undefined) data.breakMinutes = breakMinutes;

      const newStart = (data.start as Date) ?? existing.start;
      const newEnd = (data.end as Date) ?? existing.end;
      if (newEnd <= newStart) {
        return asText({ error: "end_must_be_after_start" });
      }
      if (
        !sameDay(newStart, newEnd) &&
        !(newEnd.getHours() === 0 && newEnd.getMinutes() === 0)
      ) {
        return asText({ error: "entry_must_be_same_day" });
      }

      if (tagIds !== undefined) {
        data.tags = {
          deleteMany: {},
          create: tagIds.map((tagId) => ({ tagId })),
        };
      }

      const updated = await prisma.timeEntry.update({
        where: { id },
        data,
        include: {
          project: { include: { client: true } },
          tags: { include: { tag: true } },
        },
      });
      return asText(updated);
    },
  );

  server.tool(
    "delete_entry",
    "Delete a time entry owned by the authenticated user.",
    {
      id: z.string().min(1).describe("Time entry ID to delete."),
    },
    async ({ id }) => {
      const existing = await prisma.timeEntry.findFirst({
        where: { id, userId },
      });
      if (!existing) return asText({ error: "not_found" });
      await prisma.timeEntry.delete({ where: { id } });
      return asText({ ok: true, id });
    },
  );

  server.tool(
    "get_report",
    "Aggregated time report for a week or month, grouped by client/project/tag.",
    {
      range: z.enum(["week", "month"]),
      anchor: z
        .string()
        .describe("Any ISO 8601 datetime inside the target window."),
      groupBy: z.enum(["client", "project", "tag"]),
      clientIds: z.array(z.string()).optional(),
      projectIds: z.array(z.string()).optional(),
      tagIds: z.array(z.string()).optional(),
    },
    async (args) => {
      const parsed = reportsQuerySchema.parse({
        range: args.range,
        anchor: args.anchor,
        groupBy: args.groupBy,
        clientIds: args.clientIds?.join(","),
        projectIds: args.projectIds?.join(","),
        tagIds: args.tagIds?.join(","),
      });
      const anchorDate = new Date(parsed.anchor);
      const { from, to } =
        parsed.range === "week" ? getWeekRange(anchorDate) : getMonthRange(anchorDate);

      const allEntries = await prisma.timeEntry.findMany({
        where: { userId, start: { lt: to }, end: { gt: from } },
        include: {
          project: {
            include: { client: true, tags: { include: { tag: true } } },
          },
          tags: { include: { tag: true } },
        },
      });

      const map = new Map<
        string,
        { label: string; color?: string; totalMinutes: number }
      >();
      let totalMinutes = 0;

      for (const entry of allEntries) {
        const clampedStart = entry.start < from ? from : entry.start;
        const clampedEnd = entry.end > to ? to : entry.end;
        const grossMins = Math.round(
          (clampedEnd.getTime() - clampedStart.getTime()) / 60_000,
        );
        const fullMs = entry.end.getTime() - entry.start.getTime();
        const visMs = clampedEnd.getTime() - clampedStart.getTime();
        const breakShare =
          fullMs > 0
            ? Math.round((entry.breakMinutes ?? 0) * (visMs / fullMs))
            : 0;
        const mins = Math.max(0, grossMins - breakShare);

        let key: string;
        let label: string;
        let color: string | undefined;

        if (parsed.groupBy === "tag") {
          const tagSet = new Map<string, { name: string; color: string }>();
          for (const te of entry.tags)
            tagSet.set(te.tagId, { name: te.tag.name, color: te.tag.color });
          if (entry.project) {
            for (const tp of (
              entry.project as unknown as {
                tags: { tagId: string; tag: { name: string; color: string } }[];
              }
            ).tags) {
              if (!tagSet.has(tp.tagId))
                tagSet.set(tp.tagId, { name: tp.tag.name, color: tp.tag.color });
            }
          }
          if (tagSet.size === 0) {
            const e = map.get("__none__");
            if (e) e.totalMinutes += mins;
            else
              map.set("__none__", {
                label: "タグなし",
                color: "#9ca3af",
                totalMinutes: mins,
              });
          } else {
            for (const [tagId, tag] of tagSet) {
              const e = map.get(tagId);
              if (e) e.totalMinutes += mins;
              else
                map.set(tagId, {
                  label: tag.name,
                  color: tag.color,
                  totalMinutes: mins,
                });
            }
          }
          totalMinutes += mins;
          continue;
        }

        if (!entry.project) {
          key = "__none__";
          label = "プロジェクトなし";
          color = "#9ca3af";
        } else if (parsed.groupBy === "client") {
          key = entry.project.clientId;
          label = entry.project.client.name;
        } else {
          key = entry.projectId!;
          label = `${entry.project.client.name} · ${entry.project.name}`;
          color = entry.project.color;
        }

        const e = map.get(key);
        if (e) e.totalMinutes += mins;
        else map.set(key, { label, color, totalMinutes: mins });
        totalMinutes += mins;
      }

      const rows = Array.from(map.entries()).map(([key, v]) => ({ key, ...v }));
      rows.sort((a, b) => b.totalMinutes - a.totalMinutes);

      return asText({
        rows,
        totalMinutes,
        range: { from: from.toISOString(), to: to.toISOString() },
      });
    },
  );

  return server;
}

const mcp = new Hono<{ Bindings: Env; Variables: AuthVars }>();

mcp.post("/", async (c) => {
  const userId = c.get("userId");
  const body = await c.req.json().catch(() => ({}));
  const { req, res } = toReqRes(c.req.raw);
  const server = buildServer(userId, getPrisma(c.env.DB));
  const transport = new StreamableHTTPServerTransport({
    sessionIdGenerator: undefined,
  });
  res.on("close", () => {
    transport.close();
    server.close();
  });
  await server.connect(transport);
  await transport.handleRequest(req, res, body);
  return toFetchResponse(res);
});

export { mcp };
