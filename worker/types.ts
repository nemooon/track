import type { PrismaClient } from "@prisma/client";

export type Env = {
  DB: PrismaClient;
};

export type AuthVars = {
  userId: string;
};
