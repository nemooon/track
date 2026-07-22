// DB ファイルまるごとのバックアップ / リストア。
//
// スナップショットは SQLite の VACUUM INTO で作る。単なるファイルコピーと違い
// 整合した1ファイルを吐けるので、-wal / -shm を持ち回る必要がない。つまり
// 出来上がった .db は iCloud Drive のような同期フォルダに置いても安全
// (書き込み中のファイルではないため)。
//
// 逆に、動作中の track.db / -wal / -shm を同期フォルダに置くのは破損経路。
// 3ファイルが独立に同期される、実体が退避されてプレースホルダになる、
// POSIX ロックが効かない、のいずれもが原因になる。
import { mkdirSync, readdirSync, statSync, unlinkSync, copyFileSync, existsSync } from "node:fs";
import path from "node:path";
import Database from "better-sqlite3";
import type { PrismaClient } from "@prisma/client";
import { db, closeDb, reopenDb, getDbPath } from "./dbHandle";

const AUTO_PREFIX = "track-auto-";
const MANUAL_PREFIX = "track-backup-";
const SUFFIX = ".db";

/** リストア先として最低限そろっている必要のあるテーブル */
const REQUIRED_TABLES = ["User", "Client", "Project", "TimeEntry", "Tag"];

function stamp(d: Date) {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
}

export type SnapshotInfo = {
  name: string;
  path: string;
  bytes: number;
  createdAt: string;
  auto: boolean;
};

export function listSnapshots(dir: string): SnapshotInfo[] {
  try {
    return readdirSync(dir)
      .filter(
        (f) => f.endsWith(SUFFIX) && (f.startsWith(AUTO_PREFIX) || f.startsWith(MANUAL_PREFIX)),
      )
      .map((f) => {
        const full = path.join(dir, f);
        const st = statSync(full);
        return {
          name: f,
          path: full,
          bytes: st.size,
          createdAt: new Date(st.mtimeMs).toISOString(),
          auto: f.startsWith(AUTO_PREFIX),
        };
      })
      .sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  } catch {
    return [];
  }
}

/** VACUUM INTO で整合したスナップショットを1ファイル吐く。 */
export async function snapshot(
  prisma: PrismaClient,
  dir: string,
  auto = false,
): Promise<SnapshotInfo> {
  mkdirSync(dir, { recursive: true });
  const prefix = auto ? AUTO_PREFIX : MANUAL_PREFIX;
  let file = path.join(dir, `${prefix}${stamp(new Date())}${SUFFIX}`);
  // VACUUM INTO は出力先が既にあると失敗する
  let n = 1;
  while (existsSync(file)) {
    file = path.join(dir, `${prefix}${stamp(new Date())}-${n++}${SUFFIX}`);
  }
  // パスはリテラルで埋め込むしかないので、シングルクォートだけエスケープする
  await prisma.$executeRawUnsafe(`VACUUM INTO '${file.replace(/'/g, "''")}'`);

  const st = statSync(file);
  return {
    name: path.basename(file),
    path: file,
    bytes: st.size,
    createdAt: new Date(st.mtimeMs).toISOString(),
    auto,
  };
}

/** 自動スナップショットを keep 本だけ残す。手動バックアップには触らない。 */
export function pruneSnapshots(dir: string, keep: number): number {
  const stale = listSnapshots(dir)
    .filter((s) => s.auto)
    .slice(keep);
  for (const s of stale) {
    try {
      unlinkSync(s.path);
    } catch {
      // 消せなくても致命的ではない
    }
  }
  return stale.length;
}

export async function maybeAutoBackup(
  prisma: PrismaClient,
  opts: { dir: string; intervalHours: number; keep: number },
): Promise<SnapshotInfo | null> {
  if (opts.intervalHours <= 0) return null;

  const newest = listSnapshots(opts.dir).filter((s) => s.auto)[0];
  const ageMs = newest ? Date.now() - new Date(newest.createdAt).getTime() : Infinity;
  if (ageMs < opts.intervalHours * 3_600_000) return null;

  const info = await snapshot(prisma, opts.dir, true);
  pruneSnapshots(opts.dir, opts.keep);
  return info;
}

export type ValidationResult =
  | { ok: true; counts: Record<string, number> }
  | { ok: false; reason: string };

/** 差し替える前に、それが本当に Track の DB かを確かめる。 */
export function validateSnapshot(file: string): ValidationResult {
  if (!existsSync(file)) return { ok: false, reason: "ファイルが見つかりません" };

  let sdb: Database.Database;
  try {
    sdb = new Database(file, { readonly: true, fileMustExist: true });
  } catch (e) {
    return { ok: false, reason: `SQLite として開けません: ${(e as Error).message}` };
  }

  try {
    const integrity = sdb.pragma("integrity_check", { simple: true });
    if (integrity !== "ok") return { ok: false, reason: `整合性チェックに失敗: ${integrity}` };

    const tables = new Set(
      (sdb.prepare("SELECT name FROM sqlite_master WHERE type='table'").all() as {
        name: string;
      }[]).map((r) => r.name),
    );
    const missing = REQUIRED_TABLES.filter((t) => !tables.has(t));
    if (missing.length > 0) {
      return { ok: false, reason: `Track の DB ではないようです (${missing.join(", ")} が無い)` };
    }

    const counts: Record<string, number> = {};
    for (const t of REQUIRED_TABLES) {
      counts[t] = (sdb.prepare(`SELECT COUNT(*) c FROM "${t}"`).get() as { c: number }).c;
    }
    return { ok: true, counts };
  } catch (e) {
    return { ok: false, reason: (e as Error).message };
  } finally {
    sdb.close();
  }
}

/**
 * track.db をスナップショットで置き換える。
 * 失敗しても戻せるよう、差し替え直前に現在の DB のスナップショットを取る。
 */
export async function restore(
  file: string,
  safetyDir: string,
): Promise<{ safety: SnapshotInfo | null }> {
  const target = getDbPath();

  // 差し替え前の退避。ここで失敗したらリストア自体を中止する。
  const safety = await snapshot(db(), safetyDir, false);

  await closeDb();
  try {
    copyFileSync(file, target);
    // 旧 DB の WAL が残っていると、差し替えた本体と食い違って壊れる
    for (const ext of ["-wal", "-shm"]) {
      const side = `${target}${ext}`;
      if (existsSync(side)) unlinkSync(side);
    }
  } finally {
    reopenDb();
  }

  return { safety };
}
