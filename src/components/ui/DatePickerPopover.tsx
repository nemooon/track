import { useEffect, useRef, useState } from "react";
import {
  addDays,
  addMonths,
  endOfMonth,
  endOfWeek,
  format,
  isSameDay,
  isSameMonth,
  startOfMonth,
  startOfWeek,
} from "date-fns";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { cn } from "@/lib/utils";

interface Props {
  value: Date;
  onSelect: (date: Date) => void;
  onClose: () => void;
  mode?: "day" | "week" | "month";
  className?: string;
}

const WEEKDAYS = ["日", "月", "火", "水", "木", "金", "土"];

function dowColor(dow: number, selected: boolean, inMonth: boolean, isToday: boolean) {
  if (selected) return "text-white";
  if (!inMonth) return "text-neutral-300";
  if (isToday) return "text-amber-600";
  if (dow === 0) return "text-red-500";
  if (dow === 6) return "text-blue-500";
  return "text-neutral-700";
}

export function DatePickerPopover({
  value,
  onSelect,
  onClose,
  mode = "day",
  className,
}: Props) {
  const [viewMonth, setViewMonth] = useState(() => startOfMonth(value));
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function onDocDown(e: MouseEvent) {
      if (!ref.current?.contains(e.target as Node)) onClose();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("mousedown", onDocDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDocDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  const today = new Date();

  const popoverClass = cn(
    "absolute inset-x-0 top-full z-50 mt-1 rounded-lg border border-neutral-200 bg-white p-3 shadow-lg",
    className,
  );

  if (mode === "month") {
    const year = viewMonth.getFullYear();
    const selectedYear = value.getFullYear();
    const selectedMonth = value.getMonth();
    const thisYear = today.getFullYear();
    const thisMonth = today.getMonth();
    return (
      <div ref={ref} className={popoverClass}>
        <div className="mb-2 flex items-center justify-between">
          <button
            type="button"
            onClick={() => setViewMonth((m) => addMonths(m, -12))}
            className="rounded p-1 text-neutral-600 hover:bg-neutral-100"
            aria-label="前の年"
          >
            <ChevronLeft className="size-4" strokeWidth={2.5} />
          </button>
          <span className="text-sm font-semibold tabular-nums">{year}年</span>
          <button
            type="button"
            onClick={() => setViewMonth((m) => addMonths(m, 12))}
            className="rounded p-1 text-neutral-600 hover:bg-neutral-100"
            aria-label="次の年"
          >
            <ChevronRight className="size-4" strokeWidth={2.5} />
          </button>
        </div>
        <div className="grid grid-cols-3 gap-1">
          {Array.from({ length: 12 }, (_, m) => {
            const selected = year === selectedYear && m === selectedMonth;
            const isThisMonth = year === thisYear && m === thisMonth;
            return (
              <button
                key={m}
                type="button"
                onClick={() => {
                  onSelect(new Date(year, m, 1));
                  onClose();
                }}
                className={cn(
                  "rounded-md py-2 text-sm tabular-nums transition-colors",
                  selected
                    ? "bg-neutral-900 font-semibold text-white"
                    : isThisMonth
                      ? "font-bold text-amber-600 hover:bg-amber-50"
                      : "text-neutral-700 hover:bg-neutral-100",
                )}
              >
                {m + 1}月
              </button>
            );
          })}
        </div>
      </div>
    );
  }

  const gridStart = startOfWeek(startOfMonth(viewMonth), { weekStartsOn: 0 });
  const gridEnd = endOfWeek(endOfMonth(viewMonth), { weekStartsOn: 0 });
  const days: Date[] = [];
  for (let d = gridStart; d <= gridEnd; d = addDays(d, 1)) days.push(d);
  const weeks: Date[][] = [];
  for (let i = 0; i < days.length; i += 7) weeks.push(days.slice(i, i + 7));

  const selectedWeekStart = startOfWeek(value, { weekStartsOn: 0 });

  return (
    <div ref={ref} className={popoverClass}>
      <div className="mb-2 flex items-center justify-between">
        <button
          type="button"
          onClick={() => setViewMonth((m) => addMonths(m, -1))}
          className="rounded p-1 text-neutral-600 hover:bg-neutral-100"
          aria-label="前の月"
        >
          <ChevronLeft className="size-4" strokeWidth={2.5} />
        </button>
        <span className="text-sm font-semibold tabular-nums">
          {format(viewMonth, "yyyy年 M月")}
        </span>
        <button
          type="button"
          onClick={() => setViewMonth((m) => addMonths(m, 1))}
          className="rounded p-1 text-neutral-600 hover:bg-neutral-100"
          aria-label="次の月"
        >
          <ChevronRight className="size-4" strokeWidth={2.5} />
        </button>
      </div>
      <div className="mb-1 grid grid-cols-7 gap-0.5 text-center">
        {WEEKDAYS.map((w, i) => (
          <div
            key={w}
            className={cn(
              "py-1 text-[10px] font-medium",
              i === 0 ? "text-red-500" : i === 6 ? "text-blue-500" : "text-neutral-500",
            )}
          >
            {w}
          </div>
        ))}
      </div>
      {mode === "week" ? (
        <div className="flex flex-col gap-0.5">
          {weeks.map((week) => {
            const selected = isSameDay(week[0], selectedWeekStart);
            return (
              <button
                key={week[0].toISOString()}
                type="button"
                onClick={() => {
                  onSelect(week[0]);
                  onClose();
                }}
                className={cn(
                  "grid grid-cols-7 gap-0.5 rounded-md p-0.5 text-center transition-colors",
                  selected
                    ? "bg-neutral-900 font-semibold"
                    : "hover:bg-neutral-100",
                )}
              >
                {week.map((d, dow) => {
                  const inMonth = isSameMonth(d, viewMonth);
                  const isToday = isSameDay(d, today);
                  return (
                    <span
                      key={d.toISOString()}
                      className={cn(
                        "py-1 text-xs tabular-nums",
                        dowColor(dow, selected, inMonth, isToday),
                        !selected && isToday && "font-bold",
                      )}
                    >
                      {format(d, "d")}
                    </span>
                  );
                })}
              </button>
            );
          })}
        </div>
      ) : (
        <div className="grid grid-cols-7 gap-0.5 text-center">
          {days.map((d) => {
            const inMonth = isSameMonth(d, viewMonth);
            const selected = isSameDay(d, value);
            const isToday = isSameDay(d, today);
            const dow = d.getDay();
            return (
              <button
                key={d.toISOString()}
                type="button"
                onClick={() => {
                  onSelect(d);
                  onClose();
                }}
                className={cn(
                  "rounded-md py-1.5 text-xs tabular-nums transition-colors",
                  selected
                    ? "bg-neutral-900 font-semibold text-white"
                    : "hover:bg-neutral-100",
                  !selected && dowColor(dow, false, inMonth, isToday),
                  !selected && isToday && "font-bold",
                )}
              >
                {format(d, "d")}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
