import { useState } from "react";
import {
  addDays,
  endOfMonth,
  format,
  startOfMonth,
  startOfWeek,
} from "date-fns";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { cn } from "@client/lib/utils";
import { DatePickerPopover } from "./DatePickerPopover";

export type DateRange =
  | { kind: "day" }
  | { kind: "days"; count: number }
  | { kind: "week" }
  | { kind: "month" };

interface Props {
  anchor: Date;
  range: DateRange;
  onPrev: () => void;
  onNext: () => void;
  onAnchorChange: (next: Date) => void;
  className?: string;
}

function rangeBounds(anchor: Date, range: DateRange): { start: Date; end: Date } {
  if (range.kind === "day") return { start: anchor, end: anchor };
  if (range.kind === "days") {
    return { start: anchor, end: addDays(anchor, Math.max(1, range.count) - 1) };
  }
  if (range.kind === "week") {
    const s = startOfWeek(anchor, { weekStartsOn: 0 });
    return { start: s, end: addDays(s, 6) };
  }
  return { start: startOfMonth(anchor), end: endOfMonth(anchor) };
}

function formatLabel(start: Date, end: Date, kind: DateRange["kind"]): string {
  if (kind === "month") return format(start, "M月");
  const sameDay =
    start.getFullYear() === end.getFullYear() &&
    start.getMonth() === end.getMonth() &&
    start.getDate() === end.getDate();
  if (sameDay) return format(start, "M月d日");
  const sameMonth =
    start.getFullYear() === end.getFullYear() &&
    start.getMonth() === end.getMonth();
  if (sameMonth) {
    return `${format(start, "M月d日")} – ${format(end, "d日")}`;
  }
  return `${format(start, "M月d日")} – ${format(end, "M月d日")}`;
}

export function DateRangeNavigator({
  anchor,
  range,
  onPrev,
  onNext,
  onAnchorChange,
  className,
}: Props) {
  const { start, end } = rangeBounds(anchor, range);

  const label = formatLabel(start, end, range.kind);

  const [pickerOpen, setPickerOpen] = useState(false);

  return (
    <div className={cn("relative inline-flex items-center gap-1", className)}>
      <button
        type="button"
        onClick={onPrev}
        title="前へ"
        className="inline-flex items-center justify-center rounded-md border border-neutral-200 px-2 py-1.5 text-neutral-600 hover:bg-neutral-50"
      >
        <ChevronLeft className="size-5" strokeWidth={2.5} />
      </button>
      <button
        type="button"
        onClick={() => setPickerOpen((o) => !o)}
        title="日付を選択"
        className="inline-flex w-44 items-center justify-center rounded-md border border-neutral-200 bg-white px-3 py-1.5 text-sm font-semibold tabular-nums text-neutral-900 hover:bg-neutral-50"
      >
        {label}
      </button>
      <button
        type="button"
        onClick={onNext}
        title="次へ"
        className="inline-flex items-center justify-center rounded-md border border-neutral-200 px-2 py-1.5 text-neutral-600 hover:bg-neutral-50"
      >
        <ChevronRight className="size-5" strokeWidth={2.5} />
      </button>
      {pickerOpen && (
        <DatePickerPopover
          value={anchor}
          onSelect={onAnchorChange}
          onClose={() => setPickerOpen(false)}
          mode={
            range.kind === "week"
              ? "week"
              : range.kind === "month"
                ? "month"
                : "day"
          }
        />
      )}
    </div>
  );
}
