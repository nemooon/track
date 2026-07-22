import { PrismaClient } from "@prisma/client";
import { PrismaD1 } from "@prisma/adapter-d1";

// ローカル実行時は local.ts が生成済みの PrismaClient を env.DB に直接入れる。
// ルート側は getPrisma(c.env.DB) のままでよい。
export function getPrisma(db: D1Database | PrismaClient): PrismaClient {
  if ("$transaction" in db) return db as PrismaClient;
  return new PrismaClient({ adapter: new PrismaD1(db as D1Database) });
}
