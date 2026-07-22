import { Hono } from "hono";
import { z } from "zod";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { PrismaClient } from "@prisma/client";
import { getPrisma } from "../db/prisma";
import { listSnapshots, snapshot, validateSnapshot, restore } from "../db/backup";
import type { Env } from "../types";

const data = new Hono<{ Bindings: Env }>();

export const EXPORT_VERSION = 1;

// エクスポート形式は import 側と対で維持すること。
export async function buildExport(prisma: PrismaClient) {
  const [current, clients, tags, projects, entries] = await Promise.all([
    prisma.settings.findFirst({
      select: { workStart: true, workEnd: true, workDays: true },
    }),
    prisma.client.findMany({ orderBy: { createdAt: "asc" } }),
    prisma.tag.findMany({ orderBy: { createdAt: "asc" } }),
    prisma.project.findMany({
      orderBy: { createdAt: "asc" },
      include: { tags: { select: { tagId: true } } },
    }),
    prisma.timeEntry.findMany({
      orderBy: { start: "asc" },
      include: { tags: { select: { tagId: true } } },
    }),
  ]);
  if (!current) return null;

  return {
    version: EXPORT_VERSION,
    exportedAt: new Date().toISOString(),
    settings: {
      workStart: current.workStart,
      workEnd: current.workEnd,
      workDays: current.workDays.split(",").map(Number).filter((n) => !isNaN(n)),
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
}

// GET /api/data/export — 全データを JSON で返す (curl / エージェント向け)
data.get("/export", async (c) => {
  const dump = await buildExport(getPrisma(c.env.DB));
  if (!dump) return c.json({ error: "not_found" }, 404);
  return c.json(dump);
});

// POST /api/data/export/file — サーバ側でファイルに書き出してパスを返す。
// Tauri の WebView は blob のダウンロードが素直に動かないため、
// 画面からのエクスポートはこちらを使う。
data.post("/export/file", async (c) => {
  const dump = await buildExport(getPrisma(c.env.DB));
  if (!dump) return c.json({ error: "not_found" }, 404);

  const dir = c.env.EXPORT_DIR;
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const file = path.join(
    dir,
    `track-export-${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}.json`,
  );
  mkdirSync(dir, { recursive: true });
  writeFileSync(file, JSON.stringify(dump, null, 2), "utf8");

  return c.json({
    ok: true,
    path: file,
    counts: {
      clients: dump.clients.length,
      tags: dump.tags.length,
      projects: dump.projects.length,
      entries: dump.entries.length,
    },
  });
});

const importSchema = z.object({
  version: z.literal(EXPORT_VERSION),
  settings: z.object({
    workStart: z.number().int().min(0).max(1440),
    workEnd: z.number().int().min(0).max(1440),
    workDays: z.array(z.number().int().min(0).max(6)),
  }),
  clients: z.array(
    z.object({
      id: z.string().min(1),
      name: z.string().min(1),
      archived: z.boolean().default(false),
      createdAt: z.string().datetime().optional(),
    }),
  ),
  tags: z.array(
    z.object({
      id: z.string().min(1),
      name: z.string().min(1),
      color: z.string(),
      createdAt: z.string().datetime().optional(),
    }),
  ),
  projects: z.array(
    z.object({
      id: z.string().min(1),
      clientId: z.string().min(1),
      name: z.string().min(1),
      color: z.string(),
      archived: z.boolean().default(false),
      createdAt: z.string().datetime().optional(),
      tagIds: z.array(z.string()).default([]),
    }),
  ),
  entries: z.array(
    z.object({
      id: z.string().min(1),
      projectId: z.string().nullable().default(null),
      start: z.string().datetime(),
      end: z.string().datetime(),
      title: z.string().nullable().default(null),
      note: z.string().nullable().default(null),
      externalEventId: z.string().nullable().default(null),
      externalEventSource: z.string().nullable().default(null),
      breakMinutes: z.number().int().min(0).default(0),
      createdAt: z.string().datetime().optional(),
      tagIds: z.array(z.string()).default([]),
    }),
  ),
});

// POST /api/data/import — 既存データを全消しして置き換える。
// 部分マージはせず「まるごと差し替え」だけを提供する。ID は元のまま保つので
// 同じファイルを二度入れても結果は変わらない。
data.post("/import", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = importSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const d = parsed.data;

  // 参照整合性を先に検証する。壊れたファイルで DB を空にしないため、
  // 削除より前に落とす。
  const clientIds = new Set(d.clients.map((x) => x.id));
  const tagIds = new Set(d.tags.map((x) => x.id));
  const projectIds = new Set(d.projects.map((x) => x.id));
  const problems: string[] = [];
  for (const p of d.projects) {
    if (!clientIds.has(p.clientId)) problems.push(`project ${p.id}: 未知の clientId ${p.clientId}`);
    for (const t of p.tagIds) if (!tagIds.has(t)) problems.push(`project ${p.id}: 未知の tagId ${t}`);
  }
  for (const e of d.entries) {
    if (e.projectId && !projectIds.has(e.projectId)) {
      problems.push(`entry ${e.id}: 未知の projectId ${e.projectId}`);
    }
    for (const t of e.tagIds) if (!tagIds.has(t)) problems.push(`entry ${e.id}: 未知の tagId ${t}`);
    if (new Date(e.end) <= new Date(e.start)) problems.push(`entry ${e.id}: end が start 以前`);
  }
  if (problems.length > 0) {
    return c.json({ error: "inconsistent_data", problems: problems.slice(0, 20) }, 400);
  }

  const prisma = getPrisma(c.env.DB);
  const date = (v: string | undefined) => (v ? new Date(v) : new Date());

  await prisma.$transaction(async (tx) => {
    // 削除は FK の順序に従う
    await tx.tagOnEntry.deleteMany();
    await tx.tagOnProject.deleteMany();
    await tx.timeEntry.deleteMany();
    await tx.project.deleteMany();
    await tx.client.deleteMany();
    await tx.tag.deleteMany();

    await tx.settings.updateMany({
      data: {
        workStart: d.settings.workStart,
        workEnd: d.settings.workEnd,
        workDays: d.settings.workDays.join(","),
      },
    });

    if (d.clients.length > 0) {
      await tx.client.createMany({
        data: d.clients.map((x) => ({
          id: x.id,
          name: x.name,
          archived: x.archived,
          createdAt: date(x.createdAt),
        })),
      });
    }
    if (d.tags.length > 0) {
      await tx.tag.createMany({
        data: d.tags.map((x) => ({
          id: x.id,
          name: x.name,
          color: x.color,
          createdAt: date(x.createdAt),
        })),
      });
    }
    if (d.projects.length > 0) {
      await tx.project.createMany({
        data: d.projects.map((x) => ({
          id: x.id,
          clientId: x.clientId,
          name: x.name,
          color: x.color,
          archived: x.archived,
          createdAt: date(x.createdAt),
        })),
      });
      const links = d.projects.flatMap((p) =>
        p.tagIds.map((tagId) => ({ projectId: p.id, tagId })),
      );
      if (links.length > 0) await tx.tagOnProject.createMany({ data: links });
    }
    if (d.entries.length > 0) {
      await tx.timeEntry.createMany({
        data: d.entries.map((x) => ({
          id: x.id,
          projectId: x.projectId,
          start: new Date(x.start),
          end: new Date(x.end),
          title: x.title,
          note: x.note,
          externalEventId: x.externalEventId,
          externalEventSource: x.externalEventSource,
          breakMinutes: x.breakMinutes,
          createdAt: date(x.createdAt),
        })),
      });
      const links = d.entries.flatMap((e) =>
        e.tagIds.map((tagId) => ({ entryId: e.id, tagId })),
      );
      if (links.length > 0) await tx.tagOnEntry.createMany({ data: links });
    }
  });

  return c.json({
    ok: true,
    imported: {
      clients: d.clients.length,
      tags: d.tags.length,
      projects: d.projects.length,
      entries: d.entries.length,
    },
  });
});

// --- DB ファイルまるごとのバックアップ / リストア -------------------------

// GET /api/data/backups — 保存先にあるスナップショット一覧
data.get("/backups", (c) => c.json(listSnapshots(c.env.EXPORT_DIR)));

// POST /api/data/backup — いま取る
data.post("/backup", async (c) => {
  const info = await snapshot(getPrisma(c.env.DB), c.env.EXPORT_DIR, false);
  return c.json(info);
});

const restoreSchema = z.object({ path: z.string().min(1) });

// POST /api/data/restore — track.db をスナップショットで置き換える
data.post("/restore", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = restoreSchema.safeParse(body);
  if (!parsed.success) return c.json({ error: "invalid_input" }, 400);

  const check = validateSnapshot(parsed.data.path);
  if (!check.ok) return c.json({ error: "invalid_snapshot", reason: check.reason }, 400);

  const { safety, migrated } = await restore(
    parsed.data.path,
    c.env.EXPORT_DIR,
    c.env.MIGRATIONS_DIR,
  );
  return c.json({
    ok: true,
    counts: check.counts,
    safetyBackup: safety?.path ?? null,
    migrated,
  });
});

// POST /api/data/backups/validate — 差し替え前の中身確認
data.post("/backups/validate", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = restoreSchema.safeParse(body);
  if (!parsed.success) return c.json({ error: "invalid_input" }, 400);
  return c.json(validateSnapshot(parsed.data.path));
});

export { data };
