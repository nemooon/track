import type { PrismaClient } from "@prisma/client";

export type Env = {
  DB: PrismaClient;
  /** エクスポート JSON の書き出し先 (config.json で変更可能) */
  EXPORT_DIR: string;
  /** ~/.track — config.json / DB の置き場 */
  DATA_DIR: string;
  HOME_DIR: string;
};

export type AuthVars = {
  userId: string;
};
