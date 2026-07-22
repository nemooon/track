import type { PrismaClient } from "@prisma/client";

export type Env = {
  DB: PrismaClient;
  /** エクスポート JSON の書き出し先ディレクトリ */
  EXPORT_DIR: string;
};

export type AuthVars = {
  userId: string;
};
