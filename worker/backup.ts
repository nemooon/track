// エクスポート JSON のファイル書き出しと自動バックアップ。
//
// SQLite の実体 (track.db / -wal / -shm) は iCloud Drive のような同期
// ストレージに置くと壊れる。3ファイルが独立に同期される、実体が退避されて
// プレースホルダになる、POSIX ロックが効かない、のいずれもが破損要因になる。
// そこで「DB は同期しない、エクスポートを同期する」という形にしている。
// exportDir に iCloud Drive のパスを入れれば、そこがバックアップになる。
import { mkdirSync, writeFileSync, readdirSync, statSync, unlinkSync } from "node:fs";
import path from "node:path";
import type { PrismaClient } from "@prisma/client";
import { buildExport } from "./routes/data";

const AUTO_PREFIX = "track-auto-";
const MANUAL_PREFIX = "track-export-";

function stamp(d: Date) {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}`;
}

export function writeExport(dir: string, dump: unknown, auto = false): string {
  const prefix = auto ? AUTO_PREFIX : MANUAL_PREFIX;
  const file = path.join(dir, `${prefix}${stamp(new Date())}.json`);
  mkdirSync(dir, { recursive: true });
  writeFileSync(file, JSON.stringify(dump, null, 2), "utf8");
  return file;
}

function listBackups(dir: string, prefix: string) {
  try {
    return readdirSync(dir)
      .filter((f) => f.startsWith(prefix) && f.endsWith(".json"))
      .map((f) => {
        const full = path.join(dir, f);
        return { full, mtime: statSync(full).mtimeMs };
      })
      .sort((a, b) => b.mtime - a.mtime);
  } catch {
    return [];
  }
}

/** 自動バックアップを keep 本だけ残す。手動エクスポートには触らない。 */
function prune(dir: string, keep: number) {
  const stale = listBackups(dir, AUTO_PREFIX).slice(keep);
  for (const f of stale) {
    try {
      unlinkSync(f.full);
    } catch {
      // 消せなくても致命的ではない
    }
  }
  return stale.length;
}

/**
 * 直近の自動バックアップが intervalHours より古ければ 1 本書き出す。
 * intervalHours が 0 のときは何もしない。
 */
export async function maybeAutoBackup(
  prisma: PrismaClient,
  userId: string,
  opts: { dir: string; intervalHours: number; keep: number },
): Promise<string | null> {
  if (opts.intervalHours <= 0) return null;

  const newest = listBackups(opts.dir, AUTO_PREFIX)[0];
  const ageMs = newest ? Date.now() - newest.mtime : Infinity;
  if (ageMs < opts.intervalHours * 3_600_000) return null;

  const dump = await buildExport(prisma, userId);
  if (!dump) return null;

  const file = writeExport(opts.dir, dump, true);
  prune(opts.dir, opts.keep);
  return file;
}
