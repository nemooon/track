// migrations/*.sql を順に適用する。
//
// 起動時だけでなく、リストア直後にも呼ぶ。古いスナップショットに戻したとき、
// そのファイルに入っている _local_migrations を見て足りない分だけ流すので、
// いつ取ったバックアップでも現在のスキーマに揃う。
import Database from "better-sqlite3";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

export const MIGRATIONS_DIR_NAME = "migrations";

export function runMigrations(dbPath: string, migrationsDir: string): string[] {
  const sqlite = new Database(dbPath);
  sqlite.pragma("journal_mode = WAL");
  // 0008 のようにテーブルを作り直すマイグレーションがあるため、この接続では
  // FK 制約と ALTER TABLE の参照追従を切る。適用後に整合性を確認する。
  sqlite.pragma("foreign_keys = OFF");
  sqlite.pragma("legacy_alter_table = ON");

  try {
    sqlite.exec(
      `CREATE TABLE IF NOT EXISTS "_local_migrations" (
         "name" TEXT NOT NULL PRIMARY KEY,
         "appliedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
       )`,
    );
    const applied = new Set(
      (sqlite.prepare(`SELECT name FROM "_local_migrations"`).all() as { name: string }[]).map(
        (r) => r.name,
      ),
    );
    const pending = readdirSync(migrationsDir)
      .filter((f) => f.endsWith(".sql"))
      .sort()
      .filter((f) => !applied.has(f));

    for (const file of pending) {
      const sql = readFileSync(path.join(migrationsDir, file), "utf8");
      sqlite.exec("BEGIN");
      try {
        sqlite.exec(sql);
        sqlite.prepare(`INSERT INTO "_local_migrations" (name) VALUES (?)`).run(file);
        sqlite.exec("COMMIT");
        console.log(`  migrated ${file}`);
      } catch (e) {
        sqlite.exec("ROLLBACK");
        throw new Error(`migration ${file} failed: ${(e as Error).message}`);
      }
    }

    if (pending.length > 0) {
      const violations = sqlite.pragma("foreign_key_check") as unknown[];
      if (violations.length > 0) {
        throw new Error(
          `マイグレーション後に外部キー違反が ${violations.length} 件: ${JSON.stringify(violations.slice(0, 5))}`,
        );
      }
    }
    return pending;
  } finally {
    sqlite.close();
  }
}
