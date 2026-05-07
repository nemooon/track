// Helpers to compute JST-anchored timestamps regardless of runtime timezone.
// Cloudflare Workers run in UTC; Date#setHours / getDay would shift by 9h.
const JST_OFFSET_MS = 9 * 60 * 60 * 1000;

export function jstDayOfWeek(d: Date): number {
  return new Date(d.getTime() + JST_OFFSET_MS).getUTCDay();
}

export function jstDateKey(d: Date): string {
  return new Date(d.getTime() + JST_OFFSET_MS).toISOString().slice(0, 10);
}

/** UTC Date corresponding to 00:00 JST of d's JST-local day. */
export function jstDayStart(d: Date): Date {
  const j = new Date(d.getTime() + JST_OFFSET_MS);
  const truncated = Date.UTC(j.getUTCFullYear(), j.getUTCMonth(), j.getUTCDate());
  return new Date(truncated - JST_OFFSET_MS);
}

/** ISO string for hh:mm JST on the JST-day starting at dayStart (must be a jstDayStart). */
export function jstAt(dayStart: Date, hh: number, mm: number): string {
  return new Date(dayStart.getTime() + (hh * 60 + mm) * 60_000).toISOString();
}

/** ISO string for hh:mm JST parsed from "HH:MM". */
export function jstAtHHMM(dayStart: Date, hhmm: string): string {
  const [h, m] = hhmm.split(":").map(Number);
  return jstAt(dayStart, h, m);
}

export const ONE_DAY_MS = 24 * 60 * 60 * 1000;
