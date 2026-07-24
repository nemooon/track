"use client";

import * as React from "react";
import { differenceInCalendarDays, format } from "date-fns";
import { cn } from "@client/lib/utils";
import { DAY_END_HOUR, DAY_START_HOUR } from "@shared/time";
import type { TimeEntry } from "@shared/types";
import { minutesToY, minutesFromDayStart } from "./geometry";

export function EntryBlock({
  entry,
  laneIndex,
  laneCount,
  selected,
  ghost = false,
  hourPx,
  onPointerDown,
  onTopHandleDown,
  onBottomHandleDown,
  onSelect,
  onDoubleClick,
  style,
  touchAction = "none",
}: {
  entry: TimeEntry;
  laneIndex: number;
  laneCount: number;
  selected?: boolean;
  ghost?: boolean;
  hourPx?: number;
  onPointerDown?: (e: React.PointerEvent) => void;
  onTopHandleDown?: (e: React.PointerEvent) => void;
  onBottomHandleDown?: (e: React.PointerEvent) => void;
  onSelect?: () => void;
  onDoubleClick?: () => void;
  style?: React.CSSProperties;
  touchAction?: React.CSSProperties["touchAction"];
}) {
  const start = new Date(entry.start);
  const end = new Date(entry.end);
  const startMin = minutesFromDayStart(start);
  // An end at 00:00 belongs to the next calendar day; relative to the start's
  // day it should be treated as 24:00 so the block spans to the bottom instead
  // of collapsing to a sliver right below the start.
  const endMin =
    differenceInCalendarDays(end, start) > 0
      ? (DAY_END_HOUR - DAY_START_HOUR) * 60
      : minutesFromDayStart(end);
  const top = minutesToY(startMin, hourPx) + 2;
  const height = Math.max(minutesToY(endMin - startMin, hourPx) - 5, 14);

  const width = `calc(100% / ${laneCount} - 6px)`;
  const left = `calc(${laneIndex} * 100% / ${laneCount} + 2px)`;

  const breakMin = entry.breakMinutes ?? 0;
  const grossMin = Math.round((end.getTime() - start.getTime()) / 60000);
  const durationMin = Math.max(0, grossMin - breakMin);
  const hours = Math.floor(durationMin / 60);
  const mins = durationMin % 60;
  const durLabel = hours > 0 ? `${hours}h${mins ? ` ${mins}m` : ""}` : `${mins}m`;
  const breakLabel = breakMin > 0
    ? `（休憩 ${breakMin >= 60 ? `${Math.floor(breakMin / 60)}h${breakMin % 60 ? ` ${breakMin % 60}m` : ""}` : `${breakMin}m`} 込み）`
    : "";

  const projectName = entry.project?.name ?? "プロジェクトなし";
  const baseColor = entry.project?.color ?? "#9ca3af";

  return (
    <div
      onPointerDown={onPointerDown}
      onClick={onSelect}
      onDoubleClick={onDoubleClick}
      data-selected={selected || undefined}
      className={cn(
        "calendar-entry absolute overflow-hidden rounded-md border px-2 py-1 text-xs",
        !ghost && "cursor-grab",
        ghost && "cursor-grabbing opacity-60",
      )}
      style={{
        top,
        height,
        left,
        width,
        ["--entry-accent" as never]: baseColor,
        touchAction,
        ...style,
      }}
    >
      <div
        aria-hidden="true"
        className="pointer-events-none absolute bottom-1.5 left-0 top-1.5 w-[3px] rounded-r-full bg-[var(--entry-accent)]"
      />
      {onTopHandleDown && !ghost && (
        <div
          onPointerDown={(e) => {
            e.stopPropagation();
            onTopHandleDown(e);
          }}
          className="absolute inset-x-0 top-0 h-1.5 cursor-ns-resize"
        />
      )}
      <div className="flex h-full flex-col gap-0.5 overflow-hidden">
        <div className="flex items-start gap-1.5">
          <div className="min-w-0 flex-1 truncate font-semibold leading-tight">
            {entry.title ? entry.title : projectName}
          </div>
          <div className="calendar-entry-time shrink-0 rounded px-1 py-px text-[9px] font-medium leading-tight tabular-nums">
            {format(start, "HH:mm")}–{format(end, "HH:mm")}
          </div>
        </div>
        {entry.title && (
          <div className="flex items-baseline gap-1.5 leading-tight">
            <div className="min-w-0 flex-1 truncate text-[11px] font-medium opacity-65">
              {projectName}
            </div>
            <div className="ml-auto shrink-0 text-[10px] tabular-nums opacity-55">
              {durLabel}
              {breakLabel}
            </div>
          </div>
        )}
        {(!entry.title || (entry.tags && entry.tags.length > 0)) && (
          <div className="mt-0.5 flex flex-wrap items-center gap-x-1.5 gap-y-0.5 text-[10px] leading-tight">
            {entry.tags?.map((te) => (
              <span
                key={te.tagId}
                className="inline-flex min-w-0 items-center gap-1 opacity-65"
              >
                <span
                  aria-hidden="true"
                  className="size-1.5 shrink-0 rounded-full"
                  style={{ backgroundColor: te.tag.color }}
                />
                <span className="truncate">{te.tag.name}</span>
              </span>
            ))}
            {!entry.title && (
              <span className="ml-auto tabular-nums opacity-55">
                {durLabel}
                {breakLabel}
              </span>
            )}
          </div>
        )}
      </div>
      {onBottomHandleDown && !ghost && (
        <div
          onPointerDown={(e) => {
            e.stopPropagation();
            onBottomHandleDown(e);
          }}
          className="absolute inset-x-0 bottom-0 h-1.5 cursor-ns-resize"
        />
      )}
    </div>
  );
}
