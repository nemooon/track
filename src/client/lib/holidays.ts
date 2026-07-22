import holidayJp from "@holiday-jp/holiday_jp";

export function isHoliday(date: Date): boolean {
  return holidayJp.isHoliday(date);
}

export function getHolidayName(date: Date): string | null {
  const list = holidayJp.between(date, date);
  return list.length > 0 ? list[0].name : null;
}
