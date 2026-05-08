import { DAY_END_HOUR, DAY_START_HOUR } from "@/lib/time";
import { BUFFER_PX, HOUR_PX } from "./geometry";

export function TimeGutter({ hourPx = HOUR_PX }: { hourPx?: number }) {
  const hours: number[] = [];
  for (let h = DAY_START_HOUR; h <= DAY_END_HOUR; h++) hours.push(h);

  return (
    <div
      className="relative shrink-0"
      style={{ width: 60, height: (DAY_END_HOUR - DAY_START_HOUR) * hourPx + 1 + BUFFER_PX * 2 }}
    >
      {hours.map((h) => (
        <div
          key={h}
          className="absolute right-2 -translate-y-1/2 text-[10px] text-neutral-600 tabular-nums"
          style={{ top: BUFFER_PX + (h - DAY_START_HOUR) * hourPx }}
        >
          {h.toString().padStart(2, "0")}:00
        </div>
      ))}
      {hourPx >= 96 &&
        hours.slice(0, -1).map((h) => (
          <div
            key={`${h}-30`}
            className="absolute right-2 -translate-y-1/2 text-[9px] text-neutral-400 tabular-nums"
            style={{ top: BUFFER_PX + (h - DAY_START_HOUR) * hourPx + hourPx / 2 }}
          >
            {h.toString().padStart(2, "0")}:30
          </div>
        ))}
    </div>
  );
}
