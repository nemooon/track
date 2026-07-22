import { DAY_END_HOUR, DAY_START_HOUR, SNAP_MIN } from "@shared/time";

export const HOUR_PX = 48;
export const SNAP_PX = (HOUR_PX * SNAP_MIN) / 60;
export const VISIBLE_HEIGHT = (DAY_END_HOUR - DAY_START_HOUR) * HOUR_PX;
// Fixed-pixel buffer above 0:00 and below 24:00
export const BUFFER_PX = 32;

export function snapPx(hourPx: number): number {
  return (hourPx * SNAP_MIN) / 60;
}

export function minutesFromDayStart(date: Date): number {
  return date.getHours() * 60 + date.getMinutes() - DAY_START_HOUR * 60;
}

export function minutesToY(minutes: number, hourPx = HOUR_PX): number {
  return (minutes / 60) * hourPx;
}

export function yToMinutes(y: number, hourPx = HOUR_PX): number {
  const sp = snapPx(hourPx);
  const snapped = Math.round(y / sp) * SNAP_MIN;
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
