import { useState } from "react";
import {
  addDays,
  endOfDay,
  endOfMonth,
  format,
  isWithinInterval,
  startOfDay,
  startOfMonth,
  startOfWeek,
} from "date-fns";
import { ChevronLeft, ChevronRight } from "lucide-react";
import {
  HeaderControlButton,
  HeaderControlGroup,
} from "@client/components/HeaderControls";
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
  onToday?: () => void;
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
  onToday,
  className,
}: Props) {
  const { start, end } = rangeBounds(anchor, range);

  const label = formatLabel(start, end, range.kind);
  const includesToday = isWithinInterval(new Date(), {
    start: startOfDay(start),
    end: endOfDay(end),
  });

  const [pickerOpen, setPickerOpen] = useState(false);

  return (
    <HeaderControlGroup className={`relative ${className ?? ""}`}>
      <HeaderControlButton
        onClick={onPrev}
        title="前の期間（⌘[）"
        aria-label="前の期間"
        aria-keyshortcuts="Meta+["
        iconOnly
      >
        <ChevronLeft className="size-4" strokeWidth={2.5} />
      </HeaderControlButton>
      <HeaderControlButton
        onClick={() => setPickerOpen((o) => !o)}
        title="日付を選択"
        aria-expanded={pickerOpen}
        active
        className="w-36 font-semibold tabular-nums sm:w-44"
      >
        {label}
      </HeaderControlButton>
      <HeaderControlButton
        onClick={onNext}
        title="次の期間（⌘]）"
        aria-label="次の期間"
        aria-keyshortcuts="Meta+]"
        iconOnly
      >
        <ChevronRight className="size-4" strokeWidth={2.5} />
      </HeaderControlButton>
      {onToday && (
        <HeaderControlButton
          onClick={onToday}
          title="今日（⌘T）"
          aria-keyshortcuts="Meta+T"
          aria-pressed={includesToday}
          active={includesToday}
        >
          今日
        </HeaderControlButton>
      )}
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
    </HeaderControlGroup>
  );
}
