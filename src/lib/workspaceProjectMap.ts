const STORAGE_KEY = "track:workspace-project-map";

type RepoProjectMap = Record<string, string>;

function read(): RepoProjectMap {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") return parsed as RepoProjectMap;
    return {};
  } catch {
    return {};
  }
}

function write(map: RepoProjectMap): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(map));
}

export function getRepoProjectMapping(repo: string): string | null {
  return read()[repo] ?? null;
}

export function setRepoProjectMapping(repo: string, projectId: string): void {
  const map = read();
  if (map[repo] === projectId) return;
  map[repo] = projectId;
  write(map);
}

export function removeRepoProjectMapping(repo: string): void {
  const map = read();
  if (!(repo in map)) return;
  delete map[repo];
  write(map);
}
