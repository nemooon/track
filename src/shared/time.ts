import {
  addDays,
  addMinutes,
  differenceInMinutes,
  endOfMonth,
  endOfWeek,
  startOfMonth,
  startOfWeek,
} from "date-fns";

export const SNAP_MIN = 15;
export const DAY_START_HOUR = 0;
export const DAY_END_HOUR = 24;
export const VISIBLE_HOURS = DAY_END_HOUR - DAY_START_HOUR;

export function snapToQuarter(d: Date): Date {
  const ms = 1000 * 60 * SNAP_MIN;
  return new Date(Math.round(d.getTime() / ms) * ms);
}

export function floorToQuarter(d: Date): Date {
  const ms = 1000 * 60 * SNAP_MIN;
  return new Date(Math.floor(d.getTime() / ms) * ms);
}

export function getWeekRange(anchor: Date): { from: Date; to: Date } {
  return {
    from: startOfWeek(anchor, { weekStartsOn: 0 }),
    to: addDays(startOfWeek(anchor, { weekStartsOn: 0 }), 7),
  };
}

export function getMonthRange(anchor: Date): { from: Date; to: Date } {
  return {
    from: startOfMonth(anchor),
    to: addDays(endOfMonth(anchor), 1),
  };
}

export function addMinutesSafe(d: Date, minutes: number): Date {
  return addMinutes(d, minutes);
}

export function durationMinutes(start: Date, end: Date): number {
  return differenceInMinutes(end, start);
}

export function sameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

export { addDays, endOfWeek };
