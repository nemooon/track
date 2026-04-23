import { DAY_END_HOUR, DAY_START_HOUR } from "@/lib/time";
import { HOUR_PX } from "./geometry";

export function TimeGutter({ hourPx = HOUR_PX }: { hourPx?: number }) {
  const hours: number[] = [];
  for (let h = DAY_START_HOUR; h <= DAY_END_HOUR; h++) hours.push(h);

  // Determine sub-hour tick interval based on zoom
  // hourPx >= 192 → show 15min ticks, >= 96 → 30min, else hours only
  const subInterval = hourPx >= 192 ? 15 : hourPx >= 96 ? 30 : 0;

  return (
    <div
      className="relative shrink-0"
      style={{ width: 60, height: (DAY_END_HOUR - DAY_START_HOUR) * hourPx + 1 }}
    >
      {hours.map((h) => (
        <div
          key={h}
          className="absolute right-2 -translate-y-1/2 text-[10px] text-neutral-600 tabular-nums"
          style={{ top: (h - DAY_START_HOUR) * hourPx }}
        >
          {h.toString().padStart(2, "0")}:00
        </div>
      ))}
      {subInterval > 0 &&
        hours.slice(0, -1).flatMap((h) => {
          const ticks: React.ReactNode[] = [];
          for (let m = subInterval; m < 60; m += subInterval) {
            ticks.push(
              <div
                key={`${h}-${m}`}
                className="absolute right-2 -translate-y-1/2 text-[9px] text-neutral-400 tabular-nums"
                style={{ top: (h - DAY_START_HOUR) * hourPx + (m / 60) * hourPx }}
              >
                {h.toString().padStart(2, "0")}:{m.toString().padStart(2, "0")}
              </div>,
            );
          }
          return ticks;
        })}
    </div>
  );
}
