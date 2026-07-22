import { Hono } from "hono";
import { z } from "zod";
import { existsSync, mkdirSync, accessSync, constants } from "node:fs";
import path from "node:path";
import { loadConfig, saveConfig, defaultConfig } from "../config";
import type { Env } from "../types";

const config = new Hono<{ Bindings: Env }>();

const updateSchema = z.object({
  exportDir: z.string().trim().min(1).optional(),
  backupIntervalHours: z.number().int().min(0).max(24 * 30).optional(),
  backupKeep: z.number().int().min(1).max(1000).optional(),
});

config.get("/", (c) => {
  const dir = c.env.DATA_DIR;
  return c.json({ ...loadConfig(dir), defaults: defaultConfig(dir) });
});

config.patch("/", async (c) => {
  const dataDir = c.env.DATA_DIR;
  const body = await c.req.json().catch(() => null);
  const parsed = updateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }

  const next = { ...loadConfig(dataDir), ...parsed.data };

  // 書けないパスを保存すると、次のバックアップから黙って失敗し続ける。
  // 保存前に実際に作って書ける確認をする。
  if (parsed.data.exportDir) {
    if (!path.isAbsolute(next.exportDir)) {
      return c.json({ error: "not_absolute" }, 400);
    }
    try {
      mkdirSync(next.exportDir, { recursive: true });
      accessSync(next.exportDir, constants.W_OK);
    } catch {
      return c.json({ error: "not_writable", path: next.exportDir }, 400);
    }
  }

  saveConfig(dataDir, next);
  return c.json({ ...next, defaults: defaultConfig(dataDir), applied: true });
});

// iCloud Drive のパスは環境によって存在しないので、あるときだけ候補として返す
config.get("/suggestions", (c) => {
  const home = c.env.HOME_DIR;
  const icloud = path.join(home, "Library/Mobile Documents/com~apple~CloudDocs");
  const out: { label: string; path: string }[] = [];
  if (existsSync(icloud)) {
    out.push({ label: "iCloud Drive", path: path.join(icloud, "Track") });
  }
  return c.json(out);
});

export { config };
