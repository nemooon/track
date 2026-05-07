export type WorkspaceHint = {
  id: string;
  repo: string;
  start: string;
  end: string;
  sources: { commits: number; mtimeFiles: number };
};

const EXCLUDE_DIRS = new Set([
  "node_modules",
  ".git",
  "dist",
  "build",
  ".next",
  "target",
  ".venv",
  "venv",
  "coverage",
  ".turbo",
  "out",
  ".cache",
  ".vite",
  "vendor",
]);

const MAX_DEPTH = 2;
const DEFAULT_GAP_MS = 60 * 60 * 1000;
const REPO_PARALLELISM = 4;

type Timestamps = { commits: number[]; mtimes: number[] };

async function readGitLogHead(repo: FileSystemDirectoryHandle): Promise<number[]> {
  let gitDir: FileSystemDirectoryHandle;
  try {
    gitDir = await repo.getDirectoryHandle(".git");
  } catch {
    return [];
  }
  let logsDir: FileSystemDirectoryHandle;
  try {
    logsDir = await gitDir.getDirectoryHandle("logs");
  } catch {
    return [];
  }
  let headFile: FileSystemFileHandle;
  try {
    headFile = await logsDir.getFileHandle("HEAD");
  } catch {
    return [];
  }
  const file = await headFile.getFile();
  const text = await file.text();
  const out: number[] = [];
  for (const line of text.split("\n")) {
    const tabIdx = line.indexOf("\t");
    const head = tabIdx === -1 ? line : line.slice(0, tabIdx);
    const parts = head.split(" ");
    if (parts.length < 2) continue;
    const tsStr = parts[parts.length - 2];
    const ts = Number(tsStr);
    if (Number.isFinite(ts) && ts > 0) out.push(ts * 1000);
  }
  return out;
}

async function collectMtimes(
  dir: FileSystemDirectoryHandle,
  fromMs: number,
  toMs: number,
  depth: number,
  out: number[],
): Promise<void> {
  if (depth > MAX_DEPTH) return;
  // @ts-expect-error -- async iterator on FileSystemDirectoryHandle
  for await (const [name, handle] of dir.entries() as AsyncIterable<[string, FileSystemHandle]>) {
    if (name.startsWith(".")) {
      if (name !== ".git" && name !== ".github") {
        // allow other dotfiles (they may have mtimes), but skip dot-dirs
      }
    }
    if (handle.kind === "directory") {
      if (EXCLUDE_DIRS.has(name) || name.startsWith(".")) continue;
      await collectMtimes(handle as FileSystemDirectoryHandle, fromMs, toMs, depth + 1, out);
    } else if (handle.kind === "file") {
      try {
        const file = await (handle as FileSystemFileHandle).getFile();
        const m = file.lastModified;
        if (m >= fromMs && m <= toMs) out.push(m);
      } catch {
        // ignore unreadable files
      }
    }
  }
}

function clusterTimestamps(repo: string, ts: Timestamps, gapMs: number): WorkspaceHint[] {
  const all = [...ts.commits.map((t) => ({ t, k: "c" as const })), ...ts.mtimes.map((t) => ({ t, k: "m" as const }))];
  if (all.length === 0) return [];
  all.sort((a, b) => a.t - b.t);

  const hints: WorkspaceHint[] = [];
  let groupStart = all[0].t;
  let groupEnd = all[0].t;
  let commits = 0;
  let mtimeFiles = 0;
  const flush = () => {
    hints.push({
      id: `${repo}-${groupStart}`,
      repo,
      start: new Date(groupStart).toISOString(),
      end: new Date(groupEnd).toISOString(),
      sources: { commits, mtimeFiles },
    });
  };
  for (let i = 0; i < all.length; i++) {
    const cur = all[i];
    if (i === 0) {
      groupStart = cur.t;
      groupEnd = cur.t;
      commits = cur.k === "c" ? 1 : 0;
      mtimeFiles = cur.k === "m" ? 1 : 0;
      continue;
    }
    if (cur.t - groupEnd > gapMs) {
      flush();
      groupStart = cur.t;
      groupEnd = cur.t;
      commits = cur.k === "c" ? 1 : 0;
      mtimeFiles = cur.k === "m" ? 1 : 0;
    } else {
      groupEnd = cur.t;
      if (cur.k === "c") commits++;
      else mtimeFiles++;
    }
  }
  flush();
  return hints;
}

async function scanRepo(
  name: string,
  handle: FileSystemDirectoryHandle,
  fromMs: number,
  toMs: number,
  gapMs: number,
): Promise<WorkspaceHint[]> {
  const [commits, mtimes] = await Promise.all([
    readGitLogHead(handle).then((arr) => arr.filter((t) => t >= fromMs && t <= toMs)),
    (async () => {
      const out: number[] = [];
      await collectMtimes(handle, fromMs, toMs, 0, out);
      return out;
    })(),
  ]);
  if (commits.length === 0 && mtimes.length === 0) return [];
  return clusterTimestamps(name, { commits, mtimes }, gapMs);
}

async function pMap<T, R>(items: T[], limit: number, fn: (item: T) => Promise<R>): Promise<R[]> {
  const results: R[] = new Array(items.length);
  let next = 0;
  const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (true) {
      const i = next++;
      if (i >= items.length) return;
      results[i] = await fn(items[i]);
    }
  });
  await Promise.all(workers);
  return results;
}

export async function scanWorkspace(
  root: FileSystemDirectoryHandle,
  from: Date,
  to: Date,
  options: { gapMinutes?: number } = {},
): Promise<WorkspaceHint[]> {
  const fromMs = from.getTime();
  const toMs = to.getTime();
  const gapMs = options.gapMinutes != null ? options.gapMinutes * 60 * 1000 : DEFAULT_GAP_MS;
  const repos: { name: string; handle: FileSystemDirectoryHandle }[] = [];
  // @ts-expect-error -- async iterator on FileSystemDirectoryHandle
  for await (const [name, handle] of root.entries() as AsyncIterable<[string, FileSystemHandle]>) {
    if (handle.kind !== "directory") continue;
    if (EXCLUDE_DIRS.has(name) || name.startsWith(".")) continue;
    repos.push({ name, handle: handle as FileSystemDirectoryHandle });
  }
  const nested = await pMap(repos, REPO_PARALLELISM, ({ name, handle }) =>
    scanRepo(name, handle, fromMs, toMs, gapMs).catch(() => [] as WorkspaceHint[]),
  );
  return nested.flat();
}
