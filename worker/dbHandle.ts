// PrismaClient を差し替え可能な形で保持する。
// リストアは track.db をファイルごと置き換えるため、接続を閉じてから開き直す
// 必要がある。ルート側は db() 越しに常に「いまの」クライアントを見る。
import { PrismaClient } from "@prisma/client";
import { PrismaBetterSqlite3 } from "@prisma/adapter-better-sqlite3";

let client: PrismaClient | null = null;
let dbPath = "";

function build(): PrismaClient {
  return new PrismaClient({ adapter: new PrismaBetterSqlite3({ url: `file:${dbPath}` }) });
}

export function initDb(path: string): PrismaClient {
  dbPath = path;
  client = build();
  return client;
}

export function db(): PrismaClient {
  if (!client) throw new Error("db not initialized");
  return client;
}

export function getDbPath(): string {
  return dbPath;
}

/** 接続を閉じる。ファイル差し替えの前に必ず呼ぶ。 */
export async function closeDb(): Promise<void> {
  if (!client) return;
  await client.$disconnect();
  client = null;
}

export function reopenDb(): PrismaClient {
  client = build();
  return client;
}
