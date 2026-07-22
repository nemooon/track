import type { ExternalEvent } from "@shared/types";
import { jstAt, jstAtHHMM, jstDateKey, jstDayOfWeek, jstDayStart, ONE_DAY_MS } from "./jst";

type FixturePattern = {
  dayOfWeek: 0 | 1 | 2 | 3 | 4 | 5 | 6;
  scheduleLabel?: string;
  scheduleKind?: "schedule-allday" | "schedule-halfday";
  halfdayPart?: "am" | "pm";
  clockIn?: string;
  clockOut?: string;
};

const PATTERNS: FixturePattern[] = [
  { dayOfWeek: 1, scheduleLabel: "通常勤務", scheduleKind: "schedule-allday", clockIn: "09:00", clockOut: "18:30" },
  { dayOfWeek: 2, scheduleLabel: "午前半休", scheduleKind: "schedule-halfday", halfdayPart: "am", clockIn: "13:00", clockOut: "18:15" },
  { dayOfWeek: 3, scheduleLabel: "直行直帰", scheduleKind: "schedule-allday" },
  { dayOfWeek: 4, scheduleLabel: "通常勤務", scheduleKind: "schedule-allday", clockIn: "09:15" },
  { dayOfWeek: 5, scheduleLabel: "有給休暇", scheduleKind: "schedule-allday" },
];

export function getKotEvents(from: Date, to: Date): ExternalEvent[] {
  const events: ExternalEvent[] = [];
  let cursor = jstDayStart(from);
  while (cursor < to) {
    const pattern = PATTERNS.find((p) => p.dayOfWeek === jstDayOfWeek(cursor));
    if (pattern) {
      const dayKey = jstDateKey(cursor);
      if (pattern.scheduleLabel && pattern.scheduleKind) {
        const startISO =
          pattern.scheduleKind === "schedule-halfday" && pattern.halfdayPart === "pm"
            ? jstAt(cursor, 12, 0)
            : cursor.toISOString();
        const endISO =
          pattern.scheduleKind === "schedule-halfday" && pattern.halfdayPart === "am"
            ? jstAt(cursor, 12, 0)
            : new Date(cursor.getTime() + ONE_DAY_MS).toISOString();
        events.push({
          id: `kot-sched-${dayKey}`,
          source: "kot",
          kind: pattern.scheduleKind,
          start: startISO,
          end: endISO,
          label: pattern.scheduleLabel,
          readOnly: true,
        });
      }
      if (pattern.clockIn) {
        const t = jstAtHHMM(cursor, pattern.clockIn);
        events.push({
          id: `kot-in-${dayKey}`,
          source: "kot",
          kind: "timecard-in",
          start: t,
          end: t,
          label: "出勤",
          readOnly: true,
        });
      }
      if (pattern.clockOut) {
        const t = jstAtHHMM(cursor, pattern.clockOut);
        events.push({
          id: `kot-out-${dayKey}`,
          source: "kot",
          kind: "timecard-out",
          start: t,
          end: t,
          label: "退勤",
          readOnly: true,
        });
      }
    }
    cursor = new Date(cursor.getTime() + ONE_DAY_MS);
  }
  return events;
}
