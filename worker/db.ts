import type { PrismaClient } from "@prisma/client";

// ローカル版では local.ts が生成した PrismaClient をそのまま env.DB に入れる。
// ルート側の getPrisma(c.env.DB) という書き方を変えずに済ませるためのシム。
export function getPrisma(db: PrismaClient): PrismaClient {
  return db;
}
