import "dotenv/config";
import { defineConfig, env } from "prisma/config";

// Prisma CLI configuration.
// マイグレーション/イントロスペクション用の接続 URL のみ。実行時は
// Bun サーバーが libSQL アダプタを組み立てる。
export default defineConfig({
  schema: "database/schema.prisma",
  datasource: {
    url: env("DATABASE_URL"),
  },
  migrations: {
    path: "database/migrations",
  },
});
