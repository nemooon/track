"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { format, addDays, isSameDay } from "date-fns";
import { toast } from "sonner";
import { apiFetch } from "@/lib/fetcher";
import { DAY_END_HOUR, DAY_START_HOUR, SNAP_MIN, getWeekRange } from "@/lib/time";
import type { Project, TimeEntry } from "@/types";
import { TimeGutter } from "./TimeGutter";
import { EntryBlock } from "./EntryBlock";
import { ProjectPickerPopover } from "./ProjectPickerPopover";
import {
  HOUR_PX,
  SNAP_PX,
  clampToVisible,
  makeDateAt,
  minutesFromDayStart,
  minutesToY,
  yToMinutes,
} from "./geometry";
import { layoutEntries } from "./layout";
import { useCalendarStore } from "./calendarStore";
import { cn } from "@/lib/utils";

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

export function WeekCalendar({
  anchor,
  onNavigate,
  projects,
  workStart,
  workEnd,
  workDays,
}: {
  anchor: Date;
  onNavigate: (next: Date) => void;
  projects: Project[];
  workStart?: number;
  workEnd?: number;
  workDays?: number[];
}) {
  const qc = useQueryClient();
  const { from, to } = React.useMemo(() => getWeekRange(anchor), [anchor]);
  const allDays = React.useMemo(
    () => Array.from({ length: 7 }, (_, i) => addDays(from, i)),
    [from],
  );
  const days = React.useMemo(
    () =>
      workDays
        ? allDays.filter((d) => workDays.includes(d.getDay()))
        : allDays,
    [allDays, workDays],
  );
  const weekKey = from.toISOString();

  const entriesQ = useQuery<TimeEntry[]>({
    queryKey: ["entries", weekKey],
    queryFn: () =>
      apiFetch<TimeEntry[]>(
        `/api/entries?from=${encodeURIComponent(from.toISOString())}&to=${encodeURIComponent(to.toISOString())}`,
      ),
  });

  const entries = entriesQ.data ?? [];

  const dayColRefs = React.useRef<(HTMLDivElement | null)[]>([]);
  const [interaction, setInteraction] = React.useState<Interaction>({ kind: "idle" });
  const selectedEntryId = useCalendarStore((s) => s.selectedEntryId);
  const setSelected = useCalendarStore((s) => s.setSelected);
  const [editingEntryId, setEditingEntryId] = React.useState<string | null>(null);

  // -- mutations --
  const createEntry = useMutation({
    mutationFn: (body: {
      projectId: string | null;
      start: string;
      end: string;
      title?: string;
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
    mutationFn: ({ id, body }: { id: string; body: Partial<TimeEntry> & { start?: string; end?: string } }) =>
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
    },
  });

  const deleteEntry = useMutation({
    mutationFn: (id: string) => apiFetch(`/api/entries/${id}`, { method: "DELETE" }),
    onMutate: async (id) => {
      await qc.cancelQueries({ queryKey: ["entries", weekKey] });
      const prev = qc.getQueryData<TimeEntry[]>(["entries", weekKey]);
      qc.setQueryData<TimeEntry[]>(["entries", weekKey], (old) =>
        (old ?? []).filter((e) => e.id !== id),
      );
      return { prev };
    },
    onError: (_e, _v, ctx) => {
      if (ctx?.prev) qc.setQueryData(["entries", weekKey], ctx.prev);
      toast.error("削除に失敗しました");
    },
  });

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
    target.setPointerCapture(e.pointerId);
    const y = dayColumnYFromClient(target, e.clientY);
    const minute = yToMinutes(y);
    setInteraction({
      kind: "creating",
      dayIndex,
      anchorMin: minute,
      currentMin: Math.min(minute + SNAP_MIN, (DAY_END_HOUR - DAY_START_HOUR) * 60),
    });
  }

  function onDayPointerMove(e: React.PointerEvent<HTMLDivElement>) {
    if (interaction.kind === "creating") {
      const target = e.currentTarget;
      const y = dayColumnYFromClient(target, e.clientY);
      const minute = clampToVisible(yToMinutes(y));
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
      const anchorTop = rect.top + minutesToY(endMin) + 4;
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
          yToMinutes(y) - interaction.pointerOffsetMin,
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
        const minute = clampToVisible(yToMinutes(y));
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
        updateEntry.mutate({
          id: interaction.entryId,
          body: { start: start.toISOString(), end: end.toISOString() },
        });
      }
      setInteraction({ kind: "idle" });
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [interaction]);

  function onEntryPointerDown(e: React.PointerEvent, entry: TimeEntry) {
    if (e.button !== 0) return;
    e.stopPropagation();
    setSelected(entry.id);
    const start = new Date(entry.start);
    const end = new Date(entry.end);
    const dayIndex = days.findIndex((d) => isSameDay(d, start));
    if (dayIndex < 0) return;
    const col = dayColRefs.current[dayIndex];
    if (!col) return;
    const y = dayColumnYFromClient(col, e.clientY);
    const pointerMin = yToMinutes(y);
    const pointerOffsetMin = pointerMin - minutesFromDayStart(start);
    setInteraction({
      kind: "moving",
      entryId: entry.id,
      originalStart: start,
      originalEnd: end,
      dayIndex,
      startMin: minutesFromDayStart(start),
      pointerOffsetMin,
    });
  }

  function onResizeDown(e: React.PointerEvent, entry: TimeEntry, edge: "top" | "bottom") {
    if (e.button !== 0) return;
    e.stopPropagation();
    setSelected(entry.id);
    const start = new Date(entry.start);
    const end = new Date(entry.end);
    const dayIndex = days.findIndex((d) => isSameDay(d, start));
    if (dayIndex < 0) return;
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
        const duration =
          (new Date(selected.end).getTime() - new Date(selected.start).getTime()) / 60000;
        if (duration > 120 && !confirm("この記録を削除しますか?")) return;
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
          if (newStart.getHours() < DAY_START_HOUR || newEnd.getHours() >= DAY_END_HOUR + 1) return;
          updateEntry.mutate({
            id: selected.id,
            body: { start: newStart.toISOString(), end: newEnd.toISOString() },
          });
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedEntryId, entries]);

  return (
    <div className="flex h-full flex-col">
      {/* Week navigation */}
      <div className="flex items-center gap-2 border-b border-neutral-200 px-6 py-3">
        <button
          onClick={() => onNavigate(addDays(anchor, -7))}
          className="rounded border border-neutral-200 px-2 py-1 text-sm hover:bg-neutral-50"
        >
          ‹
        </button>
        <button
          onClick={() => onNavigate(new Date())}
          className="rounded border border-neutral-200 px-3 py-1 text-sm hover:bg-neutral-50"
        >
          今週
        </button>
        <button
          onClick={() => onNavigate(addDays(anchor, 7))}
          className="rounded border border-neutral-200 px-2 py-1 text-sm hover:bg-neutral-50"
        >
          ›
        </button>
        <div className="ml-3 text-sm font-medium">
          {format(from, "yyyy/MM/dd")} – {format(addDays(from, 6), "MM/dd")}
        </div>
      </div>

      {/* Day header */}
      <div className="flex border-b border-neutral-200">
        <div style={{ width: 60 }} />
        {days.map((d) => (
          <div
            key={d.toISOString()}
            className={cn(
              "flex-1 border-l border-neutral-200 px-2 py-2 text-center text-xs",
              isSameDay(d, new Date()) && "bg-amber-50",
            )}
          >
            <div className="text-neutral-500">{format(d, "EEE")}</div>
            <div className="font-medium">{format(d, "M/d")}</div>
          </div>
        ))}
      </div>

      {/* Grid */}
      <div className="flex flex-1 overflow-auto">
        <TimeGutter />
        <div className="flex flex-1">
          {days.map((day, dayIndex) => (
            <div
              key={day.toISOString()}
              ref={(el) => {
                dayColRefs.current[dayIndex] = el;
              }}
              className="relative flex-1 border-l border-neutral-200 select-none"
              style={{
                height: (DAY_END_HOUR - DAY_START_HOUR) * HOUR_PX + 1,
                backgroundImage: `repeating-linear-gradient(to bottom, transparent 0, transparent ${HOUR_PX - 1}px, #e5e5e5 ${HOUR_PX - 1}px, #e5e5e5 ${HOUR_PX}px)`,
                touchAction: "none",
              }}
              onPointerDown={(e) => {
                if ((e.target as HTMLElement).closest("[data-entry]")) return;
                onDayPointerDown(e, dayIndex);
              }}
              onPointerMove={onDayPointerMove}
              onPointerUp={onDayPointerUp}
            >
              {/* half-hour ticks */}
              <div
                className="pointer-events-none absolute inset-0"
                style={{
                  backgroundImage: `repeating-linear-gradient(to bottom, transparent 0, transparent ${HOUR_PX / 2 - 1}px, #f5f5f5 ${HOUR_PX / 2 - 1}px, #f5f5f5 ${HOUR_PX / 2}px)`,
                  backgroundPosition: `0 ${HOUR_PX / 2}px`,
                  backgroundSize: `100% ${HOUR_PX}px`,
                }}
              />

              {/* work hours background — gray outside work range */}
              {workStart != null && workEnd != null && (
                <>
                  {workStart > DAY_START_HOUR && (
                    <div
                      className="pointer-events-none absolute inset-x-0 top-0 bg-neutral-100"
                      style={{
                        height: (workStart - DAY_START_HOUR) * HOUR_PX,
                      }}
                    />
                  )}
                  {workEnd < DAY_END_HOUR && (
                    <div
                      className="pointer-events-none absolute inset-x-0 bg-neutral-100"
                      style={{
                        top: (workEnd - DAY_START_HOUR) * HOUR_PX,
                        height: (DAY_END_HOUR - workEnd) * HOUR_PX,
                      }}
                    />
                  )}
                </>
              )}

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
                        editing={editingEntryId === entry.id && !isGhost}
                        onSelect={() => setSelected(entry.id)}
                        onDoubleClick={() => {
                          setSelected(entry.id);
                          setEditingEntryId(entry.id);
                        }}
                        onTitleCommit={(title) => {
                          setEditingEntryId(null);
                          if ((entry.title ?? "") === title) return;
                          updateEntry.mutate({
                            id: entry.id,
                            body: { title } as Partial<TimeEntry>,
                          });
                        }}
                        onEditCancel={() => setEditingEntryId(null)}
                        onPointerDown={(e) => onEntryPointerDown(e, entry)}
                        onTopHandleDown={(e) => onResizeDown(e, entry, "top")}
                        onBottomHandleDown={(e) => onResizeDown(e, entry, "bottom")}
                      />
                    </div>
                  );
                });
              })()}

              {/* drag-create ghost */}
              {interaction.kind === "creating" && interaction.dayIndex === dayIndex && (
                <div
                  className="pointer-events-none absolute inset-x-1 rounded border-2 border-dashed border-neutral-500 bg-neutral-300/30"
                  style={{
                    top: minutesToY(Math.min(interaction.anchorMin, interaction.currentMin)),
                    height: Math.max(
                      minutesToY(Math.abs(interaction.currentMin - interaction.anchorMin)),
                      2,
                    ),
                  }}
                />
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Project picker popover */}
      {interaction.kind === "picking" && (
        <ProjectPickerPopover
          anchor={interaction.anchor}
          projects={projects}
          onCancel={() => setInteraction({ kind: "idle" })}
          onPick={(projectId, title) => {
            const day = days[interaction.dayIndex];
            const start = makeDateAt(day, interaction.startMin);
            const end = makeDateAt(day, interaction.endMin);
            const trimmed = title.trim();
            createEntry.mutate({
              projectId,
              start: start.toISOString(),
              end: end.toISOString(),
              ...(trimmed ? { title: trimmed } : {}),
            });
            setInteraction({ kind: "idle" });
          }}
        />
      )}
    </div>
  );
}
