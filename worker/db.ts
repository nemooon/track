import { PrismaClient } from "@prisma/client";
import { PrismaD1 } from "@prisma/adapter-d1";

export function getPrisma(db: D1Database): PrismaClient {
  return new PrismaClient({ adapter: new PrismaD1(db) });
}
