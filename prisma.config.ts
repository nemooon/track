import "dotenv/config";
import { defineConfig, env } from "prisma/config";

// Prisma CLI configuration.
// マイグレーション/イントロスペクション用の接続 URL のみ。実行時は
// worker/local.ts が better-sqlite3 アダプタを直接組み立てる。
export default defineConfig({
  schema: "prisma/schema.prisma",
  datasource: {
    url: env("DATABASE_URL"),
  },
  migrations: {
    path: "prisma/migrations",
  },
});
