// ローカル常駐版のエントリ。Cloudflare ではなく Node で同じ Hono ルートを動かす。
//
// Web 版との違いは3点だけ:
//   1. D1 の代わりにローカルの SQLite ファイル (~/.track/track.db)
//   2. 認証なし — 単一ユーザー固定
//   3. AI 機能なし
//
// 127.0.0.1 のみで待ち受け、Origin/Host を検証して他サイトからの
// クロスオリジン書き込み (CSRF / DNS rebinding) を弾く。
import { Hono } from "hono";
import { serveStatic } from "hono/bun";
import { mkdirSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { clients } from "./routes/clients";
import { projects } from "./routes/projects";
import { entries } from "./routes/entries";
import { reports } from "./routes/reports";
import { tags } from "./routes/tags";
import { settings } from "./routes/settings";
import { config as configRoute } from "./routes/config";
import { data } from "./routes/data";
import { loadConfig } from "./config";
import { maybeAutoBackup } from "./backup";
import { initDb, db } from "./dbHandle";
import { runMigrations, MIGRATIONS_DIR_NAME } from "./migrate";
import { external } from "./routes/external";
import type { Env } from "./types";

// 本番ではTauriが同梱resourceの場所を渡す。開発時は従来どおり
// worker/local.tsからリポジトリルートを求める。
const ROOT = process.env.TRACK_RESOURCE_DIR
  ? path.resolve(process.env.TRACK_RESOURCE_DIR)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const PORT = Number(process.env.TRACK_PORT ?? 8787);
const DATA_DIR = process.env.TRACK_DATA_DIR ?? path.join(homedir(), ".track");
const DB_PATH = path.join(DATA_DIR, "track.db");

// --- DB 準備 -------------------------------------------------------------

mkdirSync(DATA_DIR, { recursive: true });

const migrated = runMigrations(DB_PATH, path.join(ROOT, MIGRATIONS_DIR_NAME));
if (migrated.length > 0) console.log(`==> ${migrated.length} 件のマイグレーションを適用`);

const prisma = initDb(DB_PATH);

// Settings は常に1行。無ければ既定値で作る。
if (!(await prisma.settings.findFirst())) {
  await prisma.settings.create({ data: {} });
  console.log("==> 設定レコードを新規作成");
}

// --- アプリ --------------------------------------------------------------

const app = new Hono<{ Bindings: Env }>();

// ローカルサーバは認証を持たないので、ブラウザ経由の他サイトからの書き込みを
// Origin / Host で弾く。curl (Origin ヘッダなし) は通る = エージェントは素通り。
const LOOPBACK = new Set(["localhost", "127.0.0.1", "[::1]", "::1"]);
const isLoopback = (hostport: string) => LOOPBACK.has(hostport.replace(/:\d+$/, ""));

app.use("*", async (c, next) => {
  // Host が攻撃者ドメインなら DNS rebinding。ループバック名以外は拒否する。
  const host = c.req.header("host");
  if (!host || !isLoopback(host)) return c.json({ error: "forbidden host" }, 403);

  // Origin があるのはブラウザからのリクエスト。他サイトのページからの
  // 書き込みを防ぐため、ループバック由来の Origin のみ許可する。
  // curl / エージェントは Origin を送らないのでそのまま通る。
  const origin = c.req.header("origin");
  if (origin) {
    let ok = false;
    try {
      const u = new URL(origin);
      ok = (u.protocol === "http:" || u.protocol === "https:") && LOOPBACK.has(u.hostname);
    } catch {
      ok = false;
    }
    if (!ok) return c.json({ error: "forbidden origin" }, 403);
  }
  await next();
});

// env を注入。全ルートは c.env しか見ていない。
app.use("*", async (c, next) => {
  c.env = {
    ...c.env,
    DB: db(),
    EXPORT_DIR: loadConfig(DATA_DIR).exportDir,
    DATA_DIR,
    HOME_DIR: homedir(),
    MIGRATIONS_DIR: path.join(ROOT, MIGRATIONS_DIR_NAME),
  } as Env;
  await next();
});

app.route("/api/clients", clients);
app.route("/api/projects", projects);
app.route("/api/entries", entries);
app.route("/api/reports", reports);
app.route("/api/tags", tags);
app.route("/api/settings", settings);
app.route("/api/data", data);
app.route("/api/config", configRoute);
app.route("/api/external", external);

app.get("/health", (c) => c.json({ ok: true, db: DB_PATH }));

// 未定義の /api/* が下の SPA catch-all に落ちると HTML が 200 で返ってしまい、
// エージェントから叩いたときに原因が分かりにくい。ここで JSON の 404 にする。
app.all("/api/*", (c) => c.json({ error: "not_found", path: c.req.path }, 404));

// ビルド済み SPA を配信 (dist/client)。無ければ Vite devサーバを使う前提でスキップ。
const DIST = path.join(ROOT, "dist/client");
if (existsSync(DIST)) {
  app.use("/assets/*", serveStatic({ root: DIST }));
  const indexFile = path.join(DIST, "index.html");
  app.get("*", () => new Response(Bun.file(indexFile)));
}

// 自動バックアップ。起動時に一度、以降は1時間ごとに「前回から
// backupIntervalHours 経ったか」を見て必要なら書き出す。
async function runAutoBackup() {
  const cfg = loadConfig(DATA_DIR);
  try {
    const info = await maybeAutoBackup(db(), {
      dir: cfg.exportDir,
      intervalHours: cfg.backupIntervalHours,
      keep: cfg.backupKeep,
    });
    if (info) console.log(`==> 自動バックアップ: ${info.path}`);
  } catch (e) {
    console.warn(`自動バックアップに失敗: ${(e as Error).message}`);
  }
}
void runAutoBackup();
setInterval(() => void runAutoBackup(), 60 * 60 * 1000);

const server = Bun.serve({ fetch: app.fetch, port: PORT, hostname: "127.0.0.1" });
console.log(`==> Track (local)  http://127.0.0.1:${server.port}`);
console.log(`    db: ${DB_PATH}`);
console.log(`    exports: ${loadConfig(DATA_DIR).exportDir}`);
if (!existsSync(DIST)) console.log(`    SPA: 未ビルド — npm run build するか vite dev を使ってください`);
