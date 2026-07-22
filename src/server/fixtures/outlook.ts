import type { ExternalEvent } from "@shared/types";
import { jstAtHHMM, jstDateKey, jstDayOfWeek, jstDayStart, ONE_DAY_MS } from "./jst";

type MeetingPattern = {
  dayOfWeek: 0 | 1 | 2 | 3 | 4 | 5 | 6;
  start: string; // "HH:MM"
  end: string;
  subject: string;
};

type AllDayPattern = {
  dayOfWeek: 0 | 1 | 2 | 3 | 4 | 5 | 6;
  subject: string;
};

const MEETINGS: MeetingPattern[] = [
  { dayOfWeek: 1, start: "10:00", end: "11:00", subject: "朝会" },
  { dayOfWeek: 1, start: "14:00", end: "15:30", subject: "案件レビュー" },
  { dayOfWeek: 2, start: "13:00", end: "14:00", subject: "1on1 / 田中さん" },
  { dayOfWeek: 3, start: "10:30", end: "11:30", subject: "設計レビュー" },
  { dayOfWeek: 3, start: "16:00", end: "17:00", subject: "部全体MTG" },
  { dayOfWeek: 4, start: "09:30", end: "10:00", subject: "デイリー" },
  { dayOfWeek: 5, start: "10:00", end: "12:00", subject: "ワークショップ" },
];

const ALLDAY: AllDayPattern[] = [
  { dayOfWeek: 3, subject: "全社会議" },
  { dayOfWeek: 5, subject: "オフサイト（社外）" },
];

export function getOutlookEvents(from: Date, to: Date): ExternalEvent[] {
  const events: ExternalEvent[] = [];
  let cursor = jstDayStart(from);
  while (cursor < to) {
    const dow = jstDayOfWeek(cursor);
    const dayKey = jstDateKey(cursor);
    for (const m of MEETINGS.filter((p) => p.dayOfWeek === dow)) {
      events.push({
        id: `outlook-mtg-${dayKey}-${m.start}`,
        source: "outlook",
        kind: "meeting",
        start: jstAtHHMM(cursor, m.start),
        end: jstAtHHMM(cursor, m.end),
        label: m.subject,
        readOnly: true,
      });
    }
    for (const a of ALLDAY.filter((p) => p.dayOfWeek === dow)) {
      events.push({
        id: `outlook-allday-${dayKey}-${a.subject}`,
        source: "outlook",
        kind: "schedule-allday",
        start: cursor.toISOString(),
        end: new Date(cursor.getTime() + ONE_DAY_MS).toISOString(),
        label: a.subject,
        readOnly: true,
      });
    }
    cursor = new Date(cursor.getTime() + ONE_DAY_MS);
  }
  return events;
}
