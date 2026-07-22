"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { format, addDays, isSameDay } from "date-fns";
import { ja } from "date-fns/locale";
import { toast } from "sonner";
import { apiFetch } from "@client/lib/fetcher";
import { DAY_END_HOUR, DAY_START_HOUR, SNAP_MIN, getWeekRange } from "@shared/time";
import { getHolidayName, isHoliday } from "@client/lib/holidays";
import type { ExternalEvent, Project, Tag, TimeEntry } from "@shared/types";
import { TimeGutter } from "./TimeGutter";
import { EntryBlock } from "./EntryBlock";
import { ProjectPickerPopover } from "./ProjectPickerPopover";
import {
  BUFFER_PX,
  clampToVisible,
  makeDateAt,
  minutesFromDayStart,
  minutesToY,
  yToMinutes,
} from "./geometry";
import { layoutBlocks, layoutEntries } from "./layout";
import { useCalendarStore, ZOOM_LEVELS } from "./calendarStore";
import { EntryEditDialog } from "./EntryEditDialog";
import { cn } from "@client/lib/utils";
import { useMediaQuery } from "@client/lib/useMediaQuery";
import { DateRangeNavigator, type DateRange } from "@client/components/ui/DateRangeNavigator";

type Interaction =
  | { kind: "idle" }
  | {
      kind: "creating";
      dayIndex: number;
      anchorMin: number;
      currentMin: number;
    }
  | {
      kind: "picking";
      dayIndex: number;
      startMin: number;
      endMin: number;
      anchor: { left: number; top: number };
      initialTitle?: string;
      externalEventId?: string;
      externalEventSource?: "kot" | "outlook";
      breakMinutes?: number;
    }
  | {
      kind: "moving";
      entryId: string;
      originalStart: Date;
      originalEnd: Date;
      dayIndex: number;
      startMin: number;
      pointerOffsetMin: number;
    }
  | {
      kind: "resizing";
      entryId: string;
      edge: "top" | "bottom";
      originalStart: Date;
      originalEnd: Date;
      startMin: number;
      endMin: number;
      dayIndex: number;
    };

type KotMeaning = "directwork" | "paidleave" | "halfday-am" | "halfday-pm" | "other";

function kotMeaning(e: ExternalEvent): KotMeaning {
  if (e.kind === "schedule-allday") {
    if (e.label.includes("直行直帰")) return "directwork";
    if (e.label.includes("有給")) return "paidleave";
  }
  if (e.kind === "schedule-halfday") {
    return new Date(e.start).getHours() === 0 ? "halfday-am" : "halfday-pm";
  }
  return "other";
}

function useCurrentMinutes() {
  const [now, setNow] = React.useState(() => new Date());
  React.useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(id);
  }, []);
  const minutes = now.getHours() * 60 + now.getMinutes() - DAY_START_HOUR * 60;
  const visible =
    minutes >= 0 && minutes <= (DAY_END_HOUR - DAY_START_HOUR) * 60;
  return { now, minutes, visible };
}

export function WeekCalendar({
  anchor,
  onNavigate,
  dayCount,
  onDayCountChange,
  projects,
  tags,
  workStart,
  workEnd,
  workDays,
}: {
  anchor: Date;
  onNavigate: (next: Date) => void;
  dayCount: 1 | 3 | "week" | 7;
  onDayCountChange: (n: 1 | 3 | "week" | 7) => void;
  projects: Project[];
  tags: Tag[];
  workStart?: number;
  workEnd?: number;
  workDays?: number[];
}) {
  const qc = useQueryClient();
  const isMobile = useMediaQuery("(max-width: 639px)");
  const isTablet = useMediaQuery("(min-width: 640px) and (max-width: 767px)");
  const effectiveDayCount: 1 | 3 | "week" | 7 = isMobile
    ? 1
    : isTablet && (dayCount === "week" || dayCount === 7)
      ? 3
      : dayCount;
  const { from, to } = React.useMemo(() => {
    if (effectiveDayCount === "week" || effectiveDayCount === 7) return getWeekRange(anchor);
    const start = new Date(anchor);
    start.setHours(0, 0, 0, 0);
    return { from: start, to: addDays(start, effectiveDayCount) };
  }, [anchor, effectiveDayCount]);
  const days = React.useMemo(() => {
    const all = Array.from({ length: 7 }, (_, i) => addDays(from, i));
    if (effectiveDayCount === "week") return workDays ? all.filter((d) => workDays.includes(d.getDay())) : all;
    if (effectiveDayCount === 7) return all;
    return Array.from({ length: effectiveDayCount }, (_, i) => addDays(from, i));
  }, [from, effectiveDayCount, workDays]);
  const weekKey = from.toISOString();

  const entriesQ = useQuery<TimeEntry[]>({
    queryKey: ["entries", weekKey],
    queryFn: () =>
      apiFetch<TimeEntry[]>(
        `/api/entries?from=${encodeURIComponent(from.toISOString())}&to=${encodeURIComponent(to.toISOString())}`,
      ),
  });

  const entries = entriesQ.data ?? [];

  const showKot = useCalendarStore((s) => s.showKot);
  const toggleShowKot = useCalendarStore((s) => s.toggleShowKot);
  const showOutlook = useCalendarStore((s) => s.showOutlook);
  const toggleShowOutlook = useCalendarStore((s) => s.toggleShowOutlook);
  const kotEventsQ = useQuery<ExternalEvent[]>({
    queryKey: ["external", "kot", weekKey, to.toISOString()],
    queryFn: () =>
      apiFetch<ExternalEvent[]>(
        `/api/external/kot/events?from=${encodeURIComponent(from.toISOString())}&to=${encodeURIComponent(to.toISOString())}`,
      ),
    enabled: showKot,
  });
  const outlookEventsQ = useQuery<ExternalEvent[]>({
    queryKey: ["external", "outlook", weekKey, to.toISOString()],
    queryFn: () =>
      apiFetch<ExternalEvent[]>(
        `/api/external/outlook/events?from=${encodeURIComponent(from.toISOString())}&to=${encodeURIComponent(to.toISOString())}`,
      ),
    enabled: showOutlook,
  });
  const linkedExternalIds = React.useMemo(
    () =>
      new Set(
        (entriesQ.data ?? [])
          .map((e) => e.externalEventId)
          .filter((id): id is string => !!id),
      ),
    [entriesQ.data],
  );
  const kotEvents = showKot
    ? (kotEventsQ.data ?? []).filter((e) => !linkedExternalIds.has(e.id))
    : [];
  const kotByDay = days.map((day) =>
    kotEvents.filter((e) => isSameDay(new Date(e.start), day)),
  );
  const outlookEvents = showOutlook
    ? (outlookEventsQ.data ?? []).filter((e) => !linkedExternalIds.has(e.id))
    : [];
  const outlookByDay = days.map((day) =>
    outlookEvents.filter((e) => isSameDay(new Date(e.start), day)),
  );
  const entryRanges = React.useMemo(
    () =>
      (entriesQ.data ?? []).map((e) => ({
        start: new Date(e.start).getTime(),
        end: new Date(e.end).getTime(),
      })),
    [entriesQ.data],
  );
  const dayColRefs = React.useRef<(HTMLDivElement | null)[]>([]);
  const [interaction, setInteraction] = React.useState<Interaction>({ kind: "idle" });
  const selectedEntryId = useCalendarStore((s) => s.selectedEntryId);
  const setSelected = useCalendarStore((s) => s.setSelected);
  const zoomIndex = useCalendarStore((s) => s.zoomIndex);
  const zoomIn = useCalendarStore((s) => s.zoomIn);
  const zoomOut = useCalendarStore((s) => s.zoomOut);
  const hourPx = ZOOM_LEVELS[zoomIndex].hourPx;
  const [dialogEntry, setDialogEntry] = React.useState<TimeEntry | null>(null);
  const [hoverState, setHoverState] = React.useState<{ dayIndex: number; minute: number } | null>(null);
  const currentTime = useCurrentMinutes();
  const scrollRef = React.useRef<HTMLDivElement>(null);
  const [scrollbarWidth, setScrollbarWidth] = React.useState(0);

  React.useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const measure = () => setScrollbarWidth(el.offsetWidth - el.clientWidth);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Scroll to a sensible initial position: 1 hour before the earlier of work start and current time
  React.useEffect(() => {
    if (!scrollRef.current) return;
    const now = new Date();
    const nowMin = now.getHours() * 60 + now.getMinutes();
    const anchorMin = workStart != null ? Math.min(workStart, nowMin) : nowMin;
    const targetMin = Math.max(0, anchorMin - 60);
    scrollRef.current.scrollTop = BUFFER_PX + minutesToY(targetMin, hourPx);
  // 初期スクロール位置を決めるだけなので、マウント時の一度きりでよい。
  // workStart や hourPx が後から変わってもユーザーの現在位置を動かさない。
  }, []);

  // -- mutations --
  const createEntry = useMutation({
    mutationFn: (body: {
      projectId: string | null;
      start: string;
      end: string;
      title?: string | null;
      note?: string | null;
      tagIds?: string[];
      externalEventId?: string;
      externalEventSource?: "kot" | "outlook";
      breakMinutes?: number;
    }) =>
      apiFetch<TimeEntry>("/api/entries", {
        method: "POST",
        body: JSON.stringify(body),
      }),
    onSuccess: (entry) => {
      qc.setQueryData<TimeEntry[]>(["entries", weekKey], (prev) =>
        prev ? [...prev, entry].sort((a, b) => a.start.localeCompare(b.start)) : [entry],
      );
    },
    onError: () => toast.error("記録の作成に失敗しました"),
  });

  const updateEntry = useMutation({
    mutationFn: ({ id, body }: { id: string; body: Partial<TimeEntry> & { start?: string; end?: string; tagIds?: string[] } }) =>
      apiFetch<TimeEntry>(`/api/entries/${id}`, {
        method: "PATCH",
        body: JSON.stringify(body),
      }),
    onMutate: async ({ id, body }) => {
      await qc.cancelQueries({ queryKey: ["entries", weekKey] });
      const prev = qc.getQueryData<TimeEntry[]>(["entries", weekKey]);
      qc.setQueryData<TimeEntry[]>(["entries", weekKey], (old) =>
        (old ?? []).map((e) => (e.id === id ? { ...e, ...body } : e)),
      );
      return { prev };
    },
    onError: (_err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(["entries", weekKey], ctx.prev);
      toast.error("更新に失敗しました");
    },
    onSuccess: (updated) => {
      qc.setQueryData<TimeEntry[]>(["entries", weekKey], (old) =>
        (old ?? []).map((e) => (e.id === updated.id ? updated : e)),
      );
      toast.success("保存しました");
    },
  });

  const deleteEntry = useMutation({
    mutationFn: (id: string) => apiFetch(`/api/entries/${id}`, { method: "DELETE" }),
    onMutate: async (id) => {
      await qc.cancelQueries({ queryKey: ["entries", weekKey] });
      const prev = qc.getQueryData<TimeEntry[]>(["entries", weekKey]);
      const removed = prev?.find((e) => e.id === id) ?? null;
      qc.setQueryData<TimeEntry[]>(["entries", weekKey], (old) =>
        (old ?? []).filter((e) => e.id !== id),
      );
      return { prev, removed };
    },
    onError: (_e, _v, ctx) => {
      if (ctx?.prev) qc.setQueryData(["entries", weekKey], ctx.prev);
      toast.error("削除に失敗しました");
    },
    onSuccess: (_data, _id, ctx) => {
      const removed = ctx?.removed;
      if (!removed) return;
      toast.success("削除しました", {
        action: {
          label: "元に戻す",
          onClick: () => {
            createEntry.mutate({
              projectId: removed.projectId,
              start: removed.start,
              end: removed.end,
              title: removed.title,
              note: removed.note,
              tagIds: removed.tags.map((t) => t.tagId),
              externalEventId: removed.externalEventId ?? undefined,
              externalEventSource: removed.externalEventSource ?? undefined,
              breakMinutes: removed.breakMinutes,
            });
          },
        },
      });
    },
  });

  // -- long-press for touch --
  const LONG_PRESS_MS = 400;
  const LONG_PRESS_MOVE_PX = 10;
  const longPressRef = React.useRef<{
    timer: ReturnType<typeof setTimeout>;
    cleanup: () => void;
  } | null>(null);

  function cancelLongPress() {
    if (longPressRef.current) {
      clearTimeout(longPressRef.current.timer);
      longPressRef.current.cleanup();
      longPressRef.current = null;
    }
  }

  function startLongPress(e: React.PointerEvent, onFire: () => void) {
    cancelLongPress();
    const { pointerId } = e;
    const startX = e.clientX;
    const startY = e.clientY;

    const handleMove = (ev: PointerEvent) => {
      if (ev.pointerId !== pointerId) return;
      const dx = ev.clientX - startX;
      const dy = ev.clientY - startY;
      if (Math.hypot(dx, dy) > LONG_PRESS_MOVE_PX) cancelLongPress();
    };
    const handleEnd = (ev: PointerEvent) => {
      if (ev.pointerId !== pointerId) return;
      cancelLongPress();
    };

    window.addEventListener("pointermove", handleMove, { passive: true });
    window.addEventListener("pointerup", handleEnd);
    window.addEventListener("pointercancel", handleEnd);

    const cleanup = () => {
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", handleEnd);
      window.removeEventListener("pointercancel", handleEnd);
    };

    longPressRef.current = {
      cleanup,
      timer: setTimeout(() => {
        const ref = longPressRef.current;
        if (!ref) return;
        longPressRef.current = null;
        ref.cleanup();
        try {
          navigator.vibrate?.(20);
        } catch {
          // ignore
        }
        onFire();
      }, LONG_PRESS_MS),
    };
  }

  React.useEffect(() => () => cancelLongPress(), []);

  // While dragging on touch, block the browser from taking over the gesture as a scroll.
  // Changing `touch-action` mid-gesture is unreliable (especially on iOS Safari), so we
  // attach a non-passive touchmove listener and preventDefault directly.
  React.useEffect(() => {
    if (interaction.kind === "idle") return;
    const handler = (e: TouchEvent) => e.preventDefault();
    document.addEventListener("touchmove", handler, { passive: false });
    return () => document.removeEventListener("touchmove", handler);
  }, [interaction.kind]);

  // -- drag handlers --
  function dayColumnYFromClient(el: HTMLElement, clientY: number): number {
    const rect = el.getBoundingClientRect();
    return clientY - rect.top;
  }

  function dayIndexFromClientX(clientX: number): number | null {
    for (let i = 0; i < dayColRefs.current.length; i++) {
      const el = dayColRefs.current[i];
      if (!el) continue;
      const r = el.getBoundingClientRect();
      if (clientX >= r.left && clientX <= r.right) return i;
    }
    return null;
  }

  function onDayPointerDown(e: React.PointerEvent<HTMLDivElement>, dayIndex: number) {
    if (e.button !== 0) return;
    const target = e.currentTarget;
    const pointerId = e.pointerId;
    const y = dayColumnYFromClient(target, e.clientY);
    const minute = yToMinutes(y, hourPx);
    const begin = () => {
      try {
        target.setPointerCapture(pointerId);
      } catch {
        // ignore
      }
      setInteraction({
        kind: "creating",
        dayIndex,
        anchorMin: minute,
        currentMin: minute,
      });
    };
    if (e.pointerType === "touch") {
      startLongPress(e, begin);
      return;
    }
    begin();
  }

  function onDayPointerMove(e: React.PointerEvent<HTMLDivElement>) {
    if (interaction.kind === "creating") {
      const target = e.currentTarget;
      const y = dayColumnYFromClient(target, e.clientY);
      const minute = clampToVisible(yToMinutes(y, hourPx));
      setInteraction({ ...interaction, currentMin: minute });
    }
  }

  function onDayPointerUp(e: React.PointerEvent<HTMLDivElement>) {
    if (interaction.kind === "creating") {
      const { dayIndex, anchorMin, currentMin } = interaction;
      const startMin = Math.min(anchorMin, currentMin);
      const endMin = Math.max(anchorMin, currentMin);
      if (endMin - startMin < SNAP_MIN) {
        setInteraction({ kind: "idle" });
        return;
      }
      const rect = dayColRefs.current[dayIndex]?.getBoundingClientRect();
      if (!rect) {
        setInteraction({ kind: "idle" });
        return;
      }
      const anchorLeft = rect.left + rect.width / 2;
      const anchorTop = rect.top + minutesToY(endMin, hourPx) + 4;
      setInteraction({
        kind: "picking",
        dayIndex,
        startMin,
        endMin,
        anchor: { left: anchorLeft, top: anchorTop },
      });
    }
  }

  // Moving: listen on window while active
  React.useEffect(() => {
    if (interaction.kind !== "moving" && interaction.kind !== "resizing") return;
    const onMove = (e: PointerEvent) => {
      if (interaction.kind === "moving") {
        const dayIdx = dayIndexFromClientX(e.clientX);
        const newDay = dayIdx ?? interaction.dayIndex;
        const col = dayColRefs.current[newDay];
        if (!col) return;
        const y = dayColumnYFromClient(col, e.clientY);
        const duration =
          minutesFromDayStart(interaction.originalEnd) -
          minutesFromDayStart(interaction.originalStart);
        const newStart = clampToVisible(
          yToMinutes(y, hourPx) - interaction.pointerOffsetMin,
        );
        const maxStart = (DAY_END_HOUR - DAY_START_HOUR) * 60 - duration;
        const clampedStart = Math.max(0, Math.min(newStart, maxStart));
        setInteraction({
          ...interaction,
          dayIndex: newDay,
          startMin: clampedStart,
        });
      } else if (interaction.kind === "resizing") {
        const col = dayColRefs.current[interaction.dayIndex];
        if (!col) return;
        const y = dayColumnYFromClient(col, e.clientY);
        const minute = clampToVisible(yToMinutes(y, hourPx));
        if (interaction.edge === "top") {
          const newStart = Math.min(minute, interaction.endMin - SNAP_MIN);
          setInteraction({ ...interaction, startMin: Math.max(0, newStart) });
        } else {
          const newEnd = Math.max(minute, interaction.startMin + SNAP_MIN);
          setInteraction({ ...interaction, endMin: newEnd });
        }
      }
    };
    const onUp = () => {
      if (interaction.kind === "moving") {
        const day = days[interaction.dayIndex];
        const duration =
          minutesFromDayStart(interaction.originalEnd) -
          minutesFromDayStart(interaction.originalStart);
        const start = makeDateAt(day, interaction.startMin);
        const end = makeDateAt(day, interaction.startMin + duration);
        if (
          start.getTime() !== interaction.originalStart.getTime() ||
          end.getTime() !== interaction.originalEnd.getTime()
        ) {
          updateEntry.mutate({
            id: interaction.entryId,
            body: { start: start.toISOString(), end: end.toISOString() },
          });
        }
      } else if (interaction.kind === "resizing") {
        const day = days[interaction.dayIndex];
        const start = makeDateAt(day, interaction.startMin);
        const end = makeDateAt(day, interaction.endMin);
        if (
          start.getTime() !== interaction.originalStart.getTime() ||
          end.getTime() !== interaction.originalEnd.getTime()
        ) {
          updateEntry.mutate({
            id: interaction.entryId,
            body: { start: start.toISOString(), end: end.toISOString() },
          });
        }
      }
      setInteraction({ kind: "idle" });
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    // days / updateEntry はドラッグ中に張り替える必要がないため依存に入れない。
    // 入れるとポインタ操作の途中でリスナーが差し替わる。
  }, [interaction, hourPx]);

  function onEntryPointerDown(e: React.PointerEvent, entry: TimeEntry) {
    if (e.button !== 0) return;
    e.stopPropagation();
    const start = new Date(entry.start);
    const end = new Date(entry.end);
    const dayIndex = days.findIndex((d) => isSameDay(d, start));
    if (dayIndex < 0) return;
    const col = dayColRefs.current[dayIndex];
    if (!col) return;
    const y = dayColumnYFromClient(col, e.clientY);
    const pointerMin = yToMinutes(y, hourPx);
    const pointerOffsetMin = pointerMin - minutesFromDayStart(start);
    const begin = () => {
      setSelected(entry.id);
      setInteraction({
        kind: "moving",
        entryId: entry.id,
        originalStart: start,
        originalEnd: end,
        dayIndex,
        startMin: minutesFromDayStart(start),
        pointerOffsetMin,
      });
    };
    if (e.pointerType === "touch") {
      startLongPress(e, begin);
      return;
    }
    begin();
  }

  function onResizeDown(e: React.PointerEvent, entry: TimeEntry, edge: "top" | "bottom") {
    if (e.button !== 0) return;
    e.stopPropagation();
    const start = new Date(entry.start);
    const end = new Date(entry.end);
    const dayIndex = days.findIndex((d) => isSameDay(d, start));
    if (dayIndex < 0) return;
    const begin = () => {
      setSelected(entry.id);
      setInteraction({
        kind: "resizing",
        entryId: entry.id,
        edge,
        originalStart: start,
        originalEnd: end,
        startMin: minutesFromDayStart(start),
        endMin: minutesFromDayStart(end),
        dayIndex,
      });
    };
    if (e.pointerType === "touch") {
      startLongPress(e, begin);
      return;
    }
    begin();
  }

  // Compose entries to render — applying in-progress transforms
  type RenderEntry = TimeEntry & { _ghost?: boolean };
  const renderedEntries: RenderEntry[] = entries.map((e) => {
    if (interaction.kind === "moving" && interaction.entryId === e.id) {
      const day = days[interaction.dayIndex];
      const duration =
        minutesFromDayStart(interaction.originalEnd) -
        minutesFromDayStart(interaction.originalStart);
      const start = makeDateAt(day, interaction.startMin);
      const end = makeDateAt(day, interaction.startMin + duration);
      return { ...e, start: start.toISOString(), end: end.toISOString(), _ghost: true };
    }
    if (interaction.kind === "resizing" && interaction.entryId === e.id) {
      const day = days[interaction.dayIndex];
      const start = makeDateAt(day, interaction.startMin);
      const end = makeDateAt(day, interaction.endMin);
      return { ...e, start: start.toISOString(), end: end.toISOString(), _ghost: true };
    }
    return e;
  });

  // Group entries by day
  const entriesByDay = days.map((day) =>
    renderedEntries.filter((e) => isSameDay(new Date(e.start), day)),
  );

  // Total worked minutes per day (excludes ghost overlap — uses each entry once).
  // breakMinutes (e.g. 60min lunch embedded in a 直行直帰 block) is excluded from worked time.
  const totalMinutesByDay = entriesByDay.map((dayEntries) =>
    dayEntries.reduce((sum, e) => {
      const ms = new Date(e.end).getTime() - new Date(e.start).getTime();
      return sum + Math.max(0, ms / 60000 - (e.breakMinutes ?? 0));
    }, 0),
  );

  // Hover indicator — follows the cursor when idle, or the active drag edge
  // during creating/moving/resizing (so long-press users get the same time feedback).
  const displayHover: { dayIndex: number; minute: number } | null = (() => {
    if (interaction.kind === "creating") {
      return { dayIndex: interaction.dayIndex, minute: interaction.currentMin };
    }
    if (interaction.kind === "moving") {
      const duration =
        minutesFromDayStart(interaction.originalEnd) -
        minutesFromDayStart(interaction.originalStart);
      return { dayIndex: interaction.dayIndex, minute: interaction.startMin + duration };
    }
    if (interaction.kind === "resizing") {
      return {
        dayIndex: interaction.dayIndex,
        minute: interaction.edge === "top" ? interaction.startMin : interaction.endMin,
      };
    }
    return interaction.kind === "idle" ? hoverState : null;
  })();

  // Keyboard shortcuts
  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const active = document.activeElement;
      if (
        active instanceof HTMLInputElement ||
        active instanceof HTMLTextAreaElement ||
        (active as HTMLElement | null)?.isContentEditable
      ) {
        return;
      }
      if (!selectedEntryId) return;
      const selected = entries.find((e) => e.id === selectedEntryId);
      if (!selected) return;

      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        deleteEntry.mutate(selected.id);
        setSelected(null);
      } else if (e.key === "Escape") {
        setSelected(null);
      } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
        e.preventDefault();
        const delta = e.key === "ArrowUp" ? -SNAP_MIN : SNAP_MIN;
        const start = new Date(selected.start);
        const end = new Date(selected.end);
        if (e.shiftKey) {
          const newEnd = new Date(end.getTime() + delta * 60000);
          if (newEnd.getTime() - start.getTime() < SNAP_MIN * 60000) return;
          updateEntry.mutate({
            id: selected.id,
            body: { end: newEnd.toISOString() },
          });
        } else {
          const newStart = new Date(start.getTime() + delta * 60000);
          const newEnd = new Date(end.getTime() + delta * 60000);
          const dayFloor = new Date(start); dayFloor.setHours(0, 0, 0, 0);
          const dayCeil = new Date(start); dayCeil.setHours(DAY_END_HOUR, 0, 0, 0);
          if (newStart < dayFloor || newEnd > dayCeil) return;
          updateEntry.mutate({
            id: selected.id,
            body: { start: newStart.toISOString(), end: newEnd.toISOString() },
          });
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // 選択中のエントリが変わったときだけ張り替えればよい。
  }, [selectedEntryId, entries]);

  return (
    <div
      className="flex h-full flex-col"
      onPointerDown={(e) => {
        if (selectedEntryId && !(e.target as HTMLElement).closest("[data-entry]")) {
          setSelected(null);
        }
      }}
    >
      {/* Week navigation */}
      <div className="flex flex-wrap items-center gap-2 border-b border-neutral-200 px-3 py-3 sm:px-6">
        {(() => {
          const isMulti = effectiveDayCount !== 1 && effectiveDayCount !== 3;
          const step = isMulti ? 7 : (effectiveDayCount as number);
          const navRange: DateRange =
            effectiveDayCount === 1
              ? { kind: "day" }
              : effectiveDayCount === 3
                ? { kind: "days", count: 3 }
                : { kind: "week" };
          return (
            <>
              <DateRangeNavigator
                anchor={anchor}
                range={navRange}
                onPrev={() => onNavigate(addDays(anchor, -step))}
                onNext={() => onNavigate(addDays(anchor, step))}
                onAnchorChange={onNavigate}
              />
              <button
                type="button"
                onClick={() => onNavigate(new Date())}
                className="inline-flex items-center justify-center rounded-md border border-neutral-200 px-3 py-1.5 text-sm text-neutral-600 hover:bg-neutral-50"
              >
                今日
              </button>
            </>
          );
        })()}
        {!isMobile && (
          <>
            <button
              onClick={toggleShowKot}
              title="KING OF TIME の打刻・スケジュール（モックデータ）を表示"
              className={cn(
                "ml-3 rounded border px-2 py-1 text-sm",
                showKot
                  ? "border-kot bg-kot/10 text-kot"
                  : "border-neutral-200 text-neutral-500 hover:bg-neutral-50",
              )}
            >
              KING OF TIME 連携（モック）
            </button>
            <button
              onClick={toggleShowOutlook}
              title="Outlook の予定（モックデータ）を表示"
              className={cn(
                "ml-1 rounded border px-2 py-1 text-sm",
                showOutlook
                  ? "border-outlook bg-outlook/10 text-outlook"
                  : "border-neutral-200 text-neutral-500 hover:bg-neutral-50",
              )}
            >
              Outlook 連携（モック）
            </button>
          </>
        )}
        <div className="ml-auto flex gap-1">
          {(
            [
              { key: 1, label: "1日", minWidth: 0 },
              { key: 3, label: "3日", minWidth: 640 },
              { key: "week", label: "稼働日", minWidth: 768 },
              { key: 7, label: "1週間", minWidth: 768 },
            ] as const
          )
            .filter(({ minWidth }) => {
              if (minWidth === 0) return true;
              if (minWidth === 640) return !isMobile;
              return !isMobile && !isTablet;
            })
            .map(({ key, label }) => (
              <button
                key={key}
                onClick={() => onDayCountChange(key)}
                className={cn(
                  "rounded border px-2 py-1 text-sm",
                  effectiveDayCount === key
                    ? "border-neutral-700 bg-neutral-700 text-white"
                    : "border-neutral-200 hover:bg-neutral-50",
                )}
              >
                {label}
              </button>
            ))}
        </div>
      </div>

      {/* Day header */}
      <div className="flex border-b border-neutral-200" style={{ paddingRight: scrollbarWidth }}>
        <div className="flex shrink-0 items-center justify-center gap-0.5" style={{ width: 60 }}>
          <button
            onClick={zoomOut}
            disabled={zoomIndex === 0}
            className="rounded size-5 text-sm text-neutral-500 font-bold leading-none hover:bg-neutral-100 disabled:opacity-30"
          >
            −
          </button>
          <button
            onClick={zoomIn}
            disabled={zoomIndex === ZOOM_LEVELS.length - 1}
            className="rounded size-5 text-sm text-neutral-500 font-bold leading-none hover:bg-neutral-100 disabled:opacity-30"
          >
            +
          </button>
        </div>
        {days.map((d, dayIndex) => {
          const holidayName = getHolidayName(d);
          const totalMin = Math.round(totalMinutesByDay[dayIndex] ?? 0);
          const h = Math.floor(totalMin / 60);
          const m = totalMin % 60;
          const durLabel = totalMin === 0 ? "" : h > 0 ? `${h}h${m ? ` ${m}m` : ""}` : `${m}m`;
          const kotBadges = kotByDay[dayIndex].filter(
            (e) => e.kind === "schedule-allday" || e.kind === "schedule-halfday",
          );
          const outlookBadges = outlookByDay[dayIndex].filter(
            (e) => e.kind === "schedule-allday",
          );
          const hasBadges = kotBadges.length > 0 || outlookBadges.length > 0;
          return (
          <div
            key={d.toISOString()}
            className={cn(
              "flex-1 border-l border-neutral-200 px-2 py-2 text-xs",
              isSameDay(d, new Date()) && "bg-amber-50",
            )}
          >
            <div className="flex items-start gap-2">
              <div title={format(d, "PPPP", { locale: ja })}>
                <div className="flex items-center justify-center size-10 rounded-full bg-neutral-200/50 text-lg font-bold">{format(d, "d")}</div>
              </div>
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <div className="flex items-center gap-2 h-5 leading-none">
                  <div className="text-neutral-500 font-semibold">{format(d, "EEEEE", { locale: ja })}</div>
                  {holidayName && (
                    <div className="rounded bg-red-400 px-1 py-0.5 text-[10px] leading-tight text-white" title={holidayName}>
                      {holidayName}
                    </div>
                  )}
                </div>
                {hasBadges && (
                  <div className="flex flex-wrap gap-1">
                    {kotBadges.map((e) => (
                      <div
                        key={e.id}
                        className="rounded bg-kot/15 px-1 py-0.5 text-[10px] leading-tight text-kot"
                        title={`KoT: ${e.label}`}
                      >
                        {e.label}
                      </div>
                    ))}
                    {outlookBadges.map((e) => (
                      <div
                        key={e.id}
                        className="rounded bg-outlook/15 px-1 py-0.5 text-[10px] leading-tight text-outlook"
                        title={`Outlook: ${e.label}`}
                      >
                        {e.label}
                      </div>
                    ))}
                  </div>
                )}
              </div>
              {durLabel && (
                <div className="self-end text-xs text-neutral-600 font-semibold tabular-nums">
                  {durLabel}
                </div>
              )}
            </div>
          </div>
          );
        })}
      </div>

      {/* Grid */}
      <div ref={scrollRef} className="flex flex-1 overflow-auto">
        <TimeGutter hourPx={hourPx} />
        <div className="flex flex-1 items-start">
          {days.map((day, dayIndex) => {
            const dayMeanings = kotByDay[dayIndex].map(kotMeaning);
            const hasPaidLeave = dayMeanings.includes("paidleave");
            const hasDirectWork = dayMeanings.includes("directwork");
            const halfdayKind: KotMeaning | null = dayMeanings.includes("halfday-am")
              ? "halfday-am"
              : dayMeanings.includes("halfday-pm")
              ? "halfday-pm"
              : null;
            const isWorkDay =
              (!workDays || workDays.includes(day.getDay())) &&
              !isHoliday(day) &&
              !hasPaidLeave;
            const hasWorkSettings = workStart != null && workEnd != null;
            return (
            <div key={day.toISOString()} className="relative flex-1 border-l border-neutral-200">
              {/* top buffer — before 0:00 */}
              <div style={{ height: BUFFER_PX, backgroundImage: "repeating-linear-gradient(45deg, oklch(92.2% 0 0) 0, oklch(92.2% 0 0) 4px, #f5f5f5 4px, #f5f5f5 10px)" }} />
            <div
              ref={(el) => {
                dayColRefs.current[dayIndex] = el;
              }}
              className="relative cursor-crosshair select-none"
              style={{
                height: (DAY_END_HOUR - DAY_START_HOUR) * hourPx + 1,
                backgroundColor: (workDays != null || hasWorkSettings) ? "oklch(92.2% 0 0 / 0.2)" : undefined,
                touchAction: interaction.kind === "idle" ? "pan-y" : "none",
              }}
              onPointerDown={(e) => {
                if ((e.target as HTMLElement).closest("[data-entry]")) return;
                onDayPointerDown(e, dayIndex);
              }}
              onPointerMove={(e) => {
                onDayPointerMove(e);
                if (interaction.kind === "idle" && e.pointerType !== "touch") {
                  const y = dayColumnYFromClient(e.currentTarget, e.clientY);
                  setHoverState({ dayIndex, minute: yToMinutes(y, hourPx) });
                }
              }}
              onPointerLeave={(e) => {
                if (interaction.kind === "idle" && e.pointerType !== "touch") setHoverState(null);
              }}
              onPointerUp={onDayPointerUp}
            >
              {/* work hours — white overlay on work days only (rendered first so grid lines appear above).
                  Half-day: 3:45 (=(workEnd-workStart-60)/2) of net work time, lunch-respecting for AM-works.
                  Otherwise: KoT clock-in shifts start; clock-out caps end. With clock-in only, end = clock-in + (workEnd - workStart). */}
              {isWorkDay && hasWorkSettings && (() => {
                if (halfdayKind) {
                  const halfWorkMin = Math.max(0, Math.floor((workEnd! - workStart! - 60) / 2));
                  const lunchStart = 12 * 60;
                  const lunchEnd = 13 * 60;
                  const blocks: { startMin: number; endMin: number }[] = [];
                  if (halfdayKind === "halfday-am") {
                    // AM is leave -> work in PM, post-lunch
                    blocks.push({ startMin: lunchEnd, endMin: lunchEnd + halfWorkMin });
                  } else {
                    // PM is leave -> work in AM, may cross lunch
                    let remaining = halfWorkMin;
                    const start = workStart!;
                    if (start < lunchStart) {
                      const beforeLunch = Math.min(remaining, lunchStart - start);
                      blocks.push({ startMin: start, endMin: start + beforeLunch });
                      remaining -= beforeLunch;
                    }
                    if (remaining > 0) {
                      const after = Math.max(start, lunchEnd);
                      blocks.push({ startMin: after, endMin: after + remaining });
                    }
                  }
                  return blocks.map((b, i) => (
                    <div
                      key={i}
                      className="pointer-events-none absolute inset-x-0 bg-white"
                      style={{
                        top: (b.startMin / 60 - DAY_START_HOUR) * hourPx,
                        height: Math.max(0, ((b.endMin - b.startMin) / 60) * hourPx),
                      }}
                    />
                  ));
                }
                const inEvt = kotByDay[dayIndex].find((e) => e.kind === "timecard-in");
                const outEvt = kotByDay[dayIndex].find((e) => e.kind === "timecard-out");
                const inMin = inEvt
                  ? new Date(inEvt.start).getHours() * 60 + new Date(inEvt.start).getMinutes()
                  : null;
                const outMin = outEvt
                  ? new Date(outEvt.start).getHours() * 60 + new Date(outEvt.start).getMinutes()
                  : null;
                const startMin = inMin ?? workStart!;
                const endMin = outMin ?? startMin + (workEnd! - workStart!);
                return (
                  <div
                    className="pointer-events-none absolute inset-x-0 bg-white"
                    style={{
                      top: (startMin / 60 - DAY_START_HOUR) * hourPx,
                      height: Math.max(0, ((endMin - startMin) / 60) * hourPx),
                    }}
                  />
                );
              })()}
              {/* 30-min ticks (dashed) */}
              <div
                className="pointer-events-none absolute inset-0"
                style={{
                  backgroundImage: `repeating-linear-gradient(to bottom, transparent 0, transparent ${hourPx / 2 - 1}px, oklch(92.2% 0 0) ${hourPx / 2 - 1}px, oklch(92.2% 0 0) ${hourPx / 2}px)`,
                  backgroundSize: `100% ${hourPx}px`,
                  backgroundPosition: `0 ${hourPx / 2}px`,
                  maskImage: `repeating-linear-gradient(to right, black 0, black 4px, transparent 2px, transparent 7px)`,
                  WebkitMaskImage: `repeating-linear-gradient(to right, black 0, black 4px, transparent 2px, transparent 7px)`,
                }}
              />
              {/* hour grid lines (darker, drawn after 30-min so they're on top) */}
              <div
                className="pointer-events-none absolute inset-0"
                style={{
                  backgroundImage: `repeating-linear-gradient(to bottom, transparent 0, transparent ${hourPx - 1}px, oklch(92.2% 0 0) ${hourPx - 1}px, oklch(92.2% 0 0) ${hourPx}px)`,
                }}
              />

              {/* External placeholders.
                  Combined into a single layer so overlapping items split into multiple lanes.
                  Rendered before entries so entries float above. */}
              {(() => {
                type Placeholder = {
                  id: string;
                  start: string;
                  end: string;
                  variant: "directwork" | "meeting";
                  label: string;
                  source?: "kot" | "outlook";
                  breakMinutes?: number;
                };
                const items: Placeholder[] = [];
                if (hasDirectWork && hasWorkSettings) {
                  const evt = kotByDay[dayIndex].find((e) => kotMeaning(e) === "directwork");
                  if (evt) {
                    const dayStart = new Date(day);
                    dayStart.setHours(0, 0, 0, 0);
                    const startISO = new Date(dayStart.getTime() + workStart! * 60_000).toISOString();
                    const endISO = new Date(dayStart.getTime() + workEnd! * 60_000).toISOString();
                    const allDayOutlook = outlookByDay[dayIndex].find((e) => e.kind === "schedule-allday");
                    items.push({
                      id: evt.id,
                      start: startISO,
                      end: endISO,
                      source: "kot",
                      variant: "directwork",
                      label: allDayOutlook?.label ?? "直行直帰",
                      breakMinutes: 60,
                    });
                  }
                }
                for (const e of outlookByDay[dayIndex]) {
                  if (e.kind !== "meeting") continue;
                  items.push({
                    id: e.id,
                    start: e.start,
                    end: e.end,
                    source: "outlook",
                    variant: "meeting",
                    label: e.label,
                  });
                }
                if (items.length === 0) return null;
                return layoutBlocks(items).map(({ item, laneIndex, laneCount }) => {
                  const startDate = new Date(item.start);
                  const endDate = new Date(item.end);
                  const startMin = minutesFromDayStart(startDate);
                  const endMin = minutesFromDayStart(endDate);
                  const width = `calc(${100 / laneCount}% - 6px)`;
                  const left = `calc(${(laneIndex / laneCount) * 100}% + 3px)`;
                  const isDirect = item.variant === "directwork";
                  const variantClass = isDirect
                    ? "border-kot bg-kot/15 text-kot hover:bg-kot/25"
                    : "border-outlook bg-outlook/15 text-outlook hover:bg-outlook/25";
                  const tooltip = isDirect
                    ? "クリックで記録を作成（8.5h・休憩1h込み = 実労 7.5h）"
                    : `クリックで記録を作成: ${item.label} (${format(startDate, "HH:mm")}–${format(endDate, "HH:mm")})`;
                  return (
                    <div
                      key={`${item.variant}-${item.id}`}
                      data-placeholder
                      className={cn(
                        "absolute z-0 cursor-pointer overflow-hidden rounded border border-dashed px-1 py-0.5 text-[10px] leading-tight",
                        variantClass,
                      )}
                      style={{
                        top: minutesToY(startMin, hourPx) + 1,
                        height: Math.max(minutesToY(endMin - startMin, hourPx) - 3, 12),
                        left,
                        width,
                      }}
                      title={tooltip}
                      onPointerDown={(ev) => ev.stopPropagation()}
                      onClick={(ev) => {
                        ev.stopPropagation();
                        const rect = (ev.currentTarget as HTMLElement).getBoundingClientRect();
                        setInteraction({
                          kind: "picking",
                          dayIndex,
                          startMin,
                          endMin,
                          anchor: { left: rect.left, top: rect.bottom + 4 },
                          initialTitle: item.label,
                          externalEventId: item.id,
                          ...(item.source ? { externalEventSource: item.source } : {}),
                          ...(item.breakMinutes ? { breakMinutes: item.breakMinutes } : {}),
                        });
                      }}
                    >
                      <div className="truncate font-medium">{item.label}</div>
                      <div className="text-[9px] tabular-nums opacity-70">
                        {format(startDate, "HH:mm")}–{format(endDate, "HH:mm")}
                        {isDirect ? "（休憩 1h 込み）" : ""}
                      </div>
                    </div>
                  );
                });
              })()}

              {/* entries */}
              {(() => {
                const laid = layoutEntries(entriesByDay[dayIndex]);
                return laid.map(({ entry, laneIndex, laneCount }) => {
                  const isGhost = (entry as RenderEntry)._ghost;
                  return (
                    <div key={entry.id} data-entry>
                      <EntryBlock
                        entry={entry}
                        laneIndex={laneIndex}
                        laneCount={laneCount}
                        selected={selectedEntryId === entry.id && !isGhost}
                        ghost={!!isGhost}
                        hourPx={hourPx}
                        touchAction={interaction.kind === "idle" ? "pan-y" : "none"}
                        onSelect={() => setSelected(entry.id)}
                        onDoubleClick={() => {
                          setSelected(entry.id);
                          setDialogEntry(entry);
                        }}
                        onPointerDown={(e) => onEntryPointerDown(e, entry)}
                        onTopHandleDown={(e) => onResizeDown(e, entry, "top")}
                        onBottomHandleDown={(e) => onResizeDown(e, entry, "bottom")}
                      />
                    </div>
                  );
                });
              })()}

              {/* KoT timecard pins — predicted clock-out when only clock-in is present */}
              {hasWorkSettings && (() => {
                const inEvt = kotByDay[dayIndex].find((e) => e.kind === "timecard-in");
                const outEvt = kotByDay[dayIndex].find((e) => e.kind === "timecard-out");
                if (!inEvt || outEvt) return null;
                const inT = new Date(inEvt.start);
                const inMin = inT.getHours() * 60 + inT.getMinutes();
                const predictedMin = inMin + (workEnd! - workStart!);
                const ph = Math.floor(predictedMin / 60);
                const pm = predictedMin % 60;
                return (
                  <div
                    className="pointer-events-none absolute inset-x-0 z-15"
                    style={{ top: minutesToY(predictedMin - DAY_START_HOUR * 60, hourPx) - 1 }}
                  >
                    <div className="absolute inset-x-0 top-0 border-t border-dashed border-kot" />
                    <div className="absolute right-1 top-px rounded-b bg-kot/80 px-1 py-0.5 text-[9px] leading-tight text-white tabular-nums">
                      予定退勤 {ph.toString().padStart(2, "0")}:{pm.toString().padStart(2, "0")}
                    </div>
                  </div>
                );
              })()}

              {/* KoT timecard pins */}
              {kotByDay[dayIndex]
                .filter((e) => e.kind === "timecard-in" || e.kind === "timecard-out")
                .map((e) => {
                  const t = new Date(e.start);
                  const minute = minutesFromDayStart(t);
                  const isIn = e.kind === "timecard-in";
                  return (
                    <div
                      key={e.id}
                      className="pointer-events-none absolute inset-x-0 z-15"
                      style={{ top: minutesToY(minute, hourPx) - 1 }}
                    >
                      <div className="absolute inset-x-0 top-0 h-px bg-kot" />
                      <div
                        className={cn(
                          "absolute bg-kot px-1 py-0.5 text-[9px] leading-tight text-white tabular-nums",
                          isIn ? "left-1 -translate-y-full rounded-t" : "right-1 top-px rounded-b",
                        )}
                      >
                        {e.label} {format(t, "HH:mm")}
                      </div>
                    </div>
                  );
                })}

              {/* current time line */}
              {currentTime.visible && isSameDay(day, currentTime.now) && (
                <div
                  className="pointer-events-none absolute inset-x-0 z-20"
                  style={{ top: minutesToY(currentTime.minutes, hourPx) }}
                >
                  <div className="absolute inset-x-0 top-0 h-px bg-red-500" />
                </div>
              )}

              {/* hover time line */}
              {displayHover?.dayIndex === dayIndex && (
                <div
                  className="pointer-events-none absolute inset-x-0 z-10"
                  style={{ top: minutesToY(displayHover.minute, hourPx) }}
                >
                  <div className="absolute inset-x-0 -top-px h-px bg-neutral-400" />
                  <div className="absolute right-full -translate-y-1/2 rounded-full bg-neutral-700 px-1 py-0.5 text-[9px] leading-tight text-white tabular-nums">
                    {(() => {
                      const total = displayHover.minute + DAY_START_HOUR * 60;
                      const h = Math.floor(total / 60);
                      const m = total % 60;
                      return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}`;
                    })()}
                  </div>
                </div>
              )}

              {/* drag-create ghost */}
              {interaction.kind === "creating" && interaction.dayIndex === dayIndex && (
                <div
                  className="pointer-events-none absolute rounded border border-dashed border-neutral-500 bg-neutral-300/30"
                  style={{
                    left: 3,
                    right: 3,
                    top: minutesToY(Math.min(interaction.anchorMin, interaction.currentMin), hourPx) + 1,
                    height: Math.max(
                      minutesToY(Math.abs(interaction.currentMin - interaction.anchorMin), hourPx) - 3,
                      2,
                    ),
                  }}
                />
              )}

              {/* picking placeholder */}
              {interaction.kind === "picking" && interaction.dayIndex === dayIndex && (
                <div
                  className="pointer-events-none absolute rounded border border-dashed border-neutral-400 bg-neutral-200/50"
                  style={{
                    left: 3,
                    right: 3,
                    top: minutesToY(interaction.startMin, hourPx) + 1,
                    height: Math.max(minutesToY(interaction.endMin - interaction.startMin, hourPx) - 3, 2),
                  }}
                />
              )}
            </div>
              {/* bottom buffer — after 24:00 */}
              <div style={{ height: BUFFER_PX, backgroundImage: "repeating-linear-gradient(45deg, oklch(92.2% 0 0) 0, oklch(92.2% 0 0) 4px, #f5f5f5 4px, #f5f5f5 10px)" }} />
            </div>
          );
          })}
        </div>
      </div>

      {/* Entry edit dialog */}
      <EntryEditDialog
        entry={dialogEntry}
        projects={projects}
        tags={tags}
        onClose={() => setDialogEntry(null)}
        onSave={(id, patch) => {
          updateEntry.mutate({ id, body: patch });
        }}
        onDelete={(id) => {
          deleteEntry.mutate(id);
          setSelected(null);
        }}
      />

      {/* Project picker popover */}
      {interaction.kind === "picking" && (
        <ProjectPickerPopover
          anchor={interaction.anchor}
          projects={projects}
          tags={tags}
          initialTitle={interaction.initialTitle}
          onCancel={() => setInteraction({ kind: "idle" })}
          onPick={(projectId, title, tagIds, note) => {
            const day = days[interaction.dayIndex];
            const start = makeDateAt(day, interaction.startMin);
            const end = makeDateAt(day, interaction.endMin);
            const trimmed = title.trim();
            const trimmedNote = note.trim();
            createEntry.mutate({
              projectId,
              start: start.toISOString(),
              end: end.toISOString(),
              ...(trimmed ? { title: trimmed } : {}),
              ...(trimmedNote ? { note: trimmedNote } : {}),
              ...(tagIds.length > 0 ? { tagIds } : {}),
              ...(interaction.externalEventId
                ? { externalEventId: interaction.externalEventId }
                : {}),
              ...(interaction.externalEventSource
                ? { externalEventSource: interaction.externalEventSource }
                : {}),
              ...(interaction.breakMinutes
                ? { breakMinutes: interaction.breakMinutes }
                : {}),
            });
            setInteraction({ kind: "idle" });
          }}
        />
      )}
    </div>
  );
}
