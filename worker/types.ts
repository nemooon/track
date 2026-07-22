import type { PrismaClient } from "@prisma/client";

export type Env = {
  DB: D1Database | PrismaClient;
  JWT_SECRET: string;
  ASSETS: Fetcher;
};

export type AuthVars = {
  userId: string;
};
