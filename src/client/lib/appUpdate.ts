const LATEST_RELEASE_URL =
  "https://api.github.com/repos/nemooon/track/releases/latest";

interface LatestReleaseResponse {
  tag_name?: unknown;
  body?: unknown;
}

export interface ReleaseNotes {
  version: string;
  body: string;
  url: string;
}

export async function getReleaseNotes(
  version: string,
  signal?: AbortSignal,
): Promise<ReleaseNotes | null> {
  const tag = `v${version}`;
  const response = await fetch(
    `https://api.github.com/repos/nemooon/track/releases/tags/${encodeURIComponent(tag)}`,
    { headers: { Accept: "application/vnd.github+json" }, signal },
  );
  if (response.status === 404) return null;
  if (!response.ok) {
    throw new Error(`GitHub Releases API: ${response.status}`);
  }
  const release = (await response.json()) as LatestReleaseResponse;
  if (release.tag_name !== tag) {
    throw new Error("リリースのバージョンが一致しません");
  }
  return {
    version,
    body: typeof release.body === "string" ? release.body.trim() : "",
    url: `https://github.com/nemooon/track/releases/tag/${encodeURIComponent(tag)}`,
  };
}

export function isNewerVersion(candidate: string, current: string) {
  const candidateParts = candidate.split(".").map((part) => Number(part) || 0);
  const currentParts = current.split(".").map((part) => Number(part) || 0);
  const length = Math.max(candidateParts.length, currentParts.length);

  for (let index = 0; index < length; index += 1) {
    const candidatePart = candidateParts[index] ?? 0;
    const currentPart = currentParts[index] ?? 0;
    if (candidatePart !== currentPart) {
      return candidatePart > currentPart;
    }
  }
  return false;
}

export async function findAvailableUpdate(
  currentVersion: string,
  signal?: AbortSignal,
) {
  const response = await fetch(LATEST_RELEASE_URL, {
    headers: { Accept: "application/vnd.github+json" },
    signal,
  });
  if (!response.ok) {
    throw new Error(`GitHub Releases API: ${response.status}`);
  }

  const release = (await response.json()) as LatestReleaseResponse;
  if (typeof release.tag_name !== "string") return null;

  const latestVersion = release.tag_name.startsWith("v")
    ? release.tag_name.slice(1)
    : release.tag_name;
  return isNewerVersion(latestVersion, currentVersion)
    ? latestVersion
    : null;
}
