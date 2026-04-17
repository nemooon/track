import "dotenv/config";
import { defineConfig, env } from "prisma/config";

// Prisma CLI configuration.
// Connection URL for migrations/introspection only — application runtime
// uses the D1 driver adapter directly (see src/lib/db.ts).
export default defineConfig({
  schema: "prisma/schema.prisma",
  datasource: {
    url: env("DATABASE_URL"),
  },
  migrations: {
    path: "prisma/migrations",
  },
});
