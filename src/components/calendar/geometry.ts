import { DAY_END_HOUR, DAY_START_HOUR, SNAP_MIN } from "@/lib/time";

export const HOUR_PX = 48;
export const SNAP_PX = (HOUR_PX * SNAP_MIN) / 60; // 12
export const VISIBLE_HEIGHT = (DAY_END_HOUR - DAY_START_HOUR) * HOUR_PX;

export function minutesFromDayStart(date: Date): number {
  return date.getHours() * 60 + date.getMinutes() - DAY_START_HOUR * 60;
}

export function minutesToY(minutes: number): number {
  return (minutes / 60) * HOUR_PX;
}

export function yToMinutes(y: number): number {
  const snapped = Math.round(y / SNAP_PX) * SNAP_MIN;
  return Math.max(0, Math.min(snapped, (DAY_END_HOUR - DAY_START_HOUR) * 60));
}

export function clampToVisible(minutes: number): number {
  const max = (DAY_END_HOUR - DAY_START_HOUR) * 60;
  return Math.max(0, Math.min(minutes, max));
}

export function makeDateAt(day: Date, minutesFromDayStart: number): Date {
  const d = new Date(day);
  d.setHours(DAY_START_HOUR, 0, 0, 0);
  d.setMinutes(d.getMinutes() + minutesFromDayStart);
  return d;
}
