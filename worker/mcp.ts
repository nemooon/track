import { Hono } from "hono";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import { toReqRes, toFetchResponse } from "fetch-to-node";
import { z } from "zod";
import { getPrisma } from "./db";
import { reportsQuerySchema } from "@/lib/validators";
import { getWeekRange, getMonthRange } from "@/lib/time";
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
