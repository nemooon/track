const LATEST_RELEASE_URL =
  "https://api.github.com/repos/nemooon/track/releases/latest";

interface LatestReleaseResponse {
  tag_name?: unknown;
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
