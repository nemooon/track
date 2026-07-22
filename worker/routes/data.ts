import { Hono } from "hono";
import { getPrisma } from "../db";
import type { Env, AuthVars } from "../types";

const data = new Hono<{ Bindings: Env; Variables: AuthVars }>();

// ローカル版 (worker/routes/data.ts on local-app) の import が受け取る形式と
// 揃えること。バージョンを上げるときは両方直す。
export const EXPORT_VERSION = 1;

// GET /api/data/export — 全データを JSON で吐く。
// D1 から手元のローカル版へ移すための一方向の経路。
// userId は内部 ID でしかないので出力には含めない。
data.get("/export", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);

  const [user, clients, tags, projects, entries] = await Promise.all([
    prisma.user.findUnique({
      where: { id: userId },
      select: { workStart: true, workEnd: true, workDays: true },
    }),
    prisma.client.findMany({ where: { userId }, orderBy: { createdAt: "asc" } }),
    prisma.tag.findMany({ where: { userId }, orderBy: { createdAt: "asc" } }),
    prisma.project.findMany({
      where: { userId },
      orderBy: { createdAt: "asc" },
      include: { tags: { select: { tagId: true } } },
    }),
    prisma.timeEntry.findMany({
      where: { userId },
      orderBy: { start: "asc" },
      include: { tags: { select: { tagId: true } } },
    }),
  ]);
  if (!user) return c.json({ error: "not_found" }, 404);

  const dump = {
    version: EXPORT_VERSION,
    exportedAt: new Date().toISOString(),
    settings: {
      workStart: user.workStart,
      workEnd: user.workEnd,
      workDays: user.workDays.split(",").map(Number).filter((n) => !isNaN(n)),
    },
    clients: clients.map((x) => ({
      id: x.id,
      name: x.name,
      archived: x.archived,
      createdAt: x.createdAt.toISOString(),
    })),
    tags: tags.map((x) => ({
      id: x.id,
      name: x.name,
      color: x.color,
      createdAt: x.createdAt.toISOString(),
    })),
    projects: projects.map((x) => ({
      id: x.id,
      clientId: x.clientId,
      name: x.name,
      color: x.color,
      archived: x.archived,
      createdAt: x.createdAt.toISOString(),
      tagIds: x.tags.map((t) => t.tagId),
    })),
    entries: entries.map((x) => ({
      id: x.id,
      projectId: x.projectId,
      start: x.start.toISOString(),
      end: x.end.toISOString(),
      title: x.title,
      note: x.note,
      externalEventId: x.externalEventId,
      externalEventSource: x.externalEventSource,
      breakMinutes: x.breakMinutes,
      createdAt: x.createdAt.toISOString(),
      tagIds: x.tags.map((t) => t.tagId),
    })),
  };

  // ブラウザで開いたらそのままファイルとして落ちるようにする。
  // パスキーでログイン済みなら URL を叩くだけで済み、PAT を用意しなくてよい。
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const filename = `track-export-${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}.json`;

  return new Response(JSON.stringify(dump, null, 2), {
    headers: {
      "content-type": "application/json; charset=utf-8",
      "content-disposition": `attachment; filename="${filename}"`,
    },
  });
});

export { data };
