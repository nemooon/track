import { DAY_END_HOUR, DAY_START_HOUR } from "@/lib/time";
import { HOUR_PX } from "./geometry";

export function TimeGutter() {
  const hours: number[] = [];
  for (let h = DAY_START_HOUR; h <= DAY_END_HOUR; h++) hours.push(h);
  return (
    <div
      className="relative shrink-0"
      style={{ width: 60, height: (DAY_END_HOUR - DAY_START_HOUR) * HOUR_PX + 1 }}
    >
      {hours.map((h) => (
        <div
          key={h}
          className="absolute right-2 -translate-y-1/2 text-[10px] text-neutral-400 tabular-nums"
          style={{ top: (h - DAY_START_HOUR) * HOUR_PX }}
        >
          {h.toString().padStart(2, "0")}:00
        </div>
      ))}
    </div>
  );
}
