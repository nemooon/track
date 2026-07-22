// ローカル常駐版のエントリ。Cloudflare ではなく Node で同じ Hono ルートを動かす。
//
// Web 版との違いは3点だけ:
//   1. D1 の代わりにローカルの SQLite ファイル (~/.track/track.db)
//   2. 認証なし — 単一ユーザー固定
//   3. AI 機能なし
//
// 127.0.0.1 のみで待ち受け、Origin/Host を検証して他サイトからの
// クロスオリジン書き込み (CSRF / DNS rebinding) を弾く。
import { serve } from "@hono/node-server";
import { serveStatic } from "@hono/node-server/serve-static";
import { Hono } from "hono";
import { PrismaClient } from "@prisma/client";
import { PrismaBetterSqlite3 } from "@prisma/adapter-better-sqlite3";
import Database from "better-sqlite3";
import { readFileSync, readdirSync, mkdirSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { clients } from "./routes/clients";
import { projects } from "./routes/projects";
import { entries } from "./routes/entries";
import { reports } from "./routes/reports";
import { tags } from "./routes/tags";
import { settings } from "./routes/settings";
import { data } from "./routes/data";
import { external } from "./routes/external";
import type { Env, AuthVars } from "./types";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const PORT = Number(process.env.TRACK_PORT ?? 8787);
const DATA_DIR = process.env.TRACK_DATA_DIR ?? path.join(homedir(), ".track");
const DB_PATH = path.join(DATA_DIR, "track.db");
const EXPORT_DIR = path.join(DATA_DIR, "exports");

// --- DB 準備 -------------------------------------------------------------

mkdirSync(DATA_DIR, { recursive: true });
const sqlite = new Database(DB_PATH);
// GUI と CLI/エージェントが同時に触るので WAL 必須。
sqlite.pragma("journal_mode = WAL");
sqlite.pragma("foreign_keys = ON");

function migrate() {
  sqlite.exec(
    `CREATE TABLE IF NOT EXISTS "_local_migrations" (
       "name" TEXT NOT NULL PRIMARY KEY,
       "appliedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
     )`,
  );
  const applied = new Set(
    (sqlite.prepare(`SELECT name FROM "_local_migrations"`).all() as { name: string }[]).map(
      (r) => r.name,
    ),
  );
  const dir = path.join(ROOT, "migrations");
  const pending = readdirSync(dir)
    .filter((f) => f.endsWith(".sql"))
    .sort()
    .filter((f) => !applied.has(f));

  for (const file of pending) {
    const sql = readFileSync(path.join(dir, file), "utf8");
    sqlite.exec("BEGIN");
    try {
      sqlite.exec(sql);
      sqlite.prepare(`INSERT INTO "_local_migrations" (name) VALUES (?)`).run(file);
      sqlite.exec("COMMIT");
      console.log(`  migrated ${file}`);
    } catch (e) {
      sqlite.exec("ROLLBACK");
      throw new Error(`migration ${file} failed: ${(e as Error).message}`);
    }
  }
  return pending.length;
}

const migrated = migrate();
if (migrated > 0) console.log(`==> ${migrated} 件のマイグレーションを適用`);
// マイグレーション専用の接続はここで閉じる。以降は Prisma 側が自前で開く。
sqlite.close();

const prisma = new PrismaClient({ adapter: new PrismaBetterSqlite3({ url: `file:${DB_PATH}` }) });

// 単一ユーザーを確定する。既存行があればそれを使う (db:pull したデータを尊重)。
async function resolveOwnerId(): Promise<string> {
  const existing = await prisma.user.findFirst({ orderBy: { createdAt: "asc" } });
  if (existing) return existing.id;
  const created = await prisma.user.create({ data: {} });
  console.log(`==> 設定レコードを新規作成`);
  return created.id;
}
const OWNER_ID = await resolveOwnerId();

// --- アプリ --------------------------------------------------------------

const app = new Hono<{ Bindings: Env; Variables: AuthVars }>();

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

// env / userId を注入。全ルートは c.env.DB と c.get("userId") しか見ていない。
app.use("*", async (c, next) => {
  c.env = { ...c.env, DB: prisma, EXPORT_DIR } as Env;
  c.set("userId", OWNER_ID);
  await next();
});

app.route("/api/clients", clients);
app.route("/api/projects", projects);
app.route("/api/entries", entries);
app.route("/api/reports", reports);
app.route("/api/tags", tags);
app.route("/api/settings", settings);
app.route("/api/data", data);
app.route("/api/external", external);

app.get("/health", (c) => c.json({ ok: true, db: DB_PATH, userId: OWNER_ID }));

// 未定義の /api/* が下の SPA catch-all に落ちると HTML が 200 で返ってしまい、
// エージェントから叩いたときに原因が分かりにくい。ここで JSON の 404 にする。
app.all("/api/*", (c) => c.json({ error: "not_found", path: c.req.path }, 404));

// ビルド済み SPA を配信 (dist/client)。無ければ Vite devサーバを使う前提でスキップ。
const DIST = path.join(ROOT, "dist/client");
if (existsSync(DIST)) {
  app.use("/assets/*", serveStatic({ root: path.relative(process.cwd(), DIST) }));
  app.get("*", serveStatic({ path: path.relative(process.cwd(), path.join(DIST, "index.html")) }));
}

serve({ fetch: app.fetch, port: PORT, hostname: "127.0.0.1" }, (info) => {
  console.log(`==> Track (local)  http://127.0.0.1:${info.port}`);
  console.log(`    db: ${DB_PATH}`);
  console.log(`    exports: ${EXPORT_DIR}`);
  if (!existsSync(DIST)) console.log(`    SPA: 未ビルド — npm run build するか vite dev を使ってください`);
});
