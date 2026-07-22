import type { PrismaClient } from "@prisma/client";

export type Env = {
  DB: D1Database | PrismaClient;
  JWT_SECRET: string;
  ASSETS: Fetcher;
  CF_ACCOUNT_ID?: string;
  CF_API_TOKEN?: string;
};

export type AuthVars = {
  userId: string;
};
