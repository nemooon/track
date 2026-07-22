"use client";

import * as React from "react";
import { differenceInCalendarDays, format } from "date-fns";
import { cn } from "@/lib/utils";
import { DAY_END_HOUR, DAY_START_HOUR } from "@/lib/time";
import type { TimeEntry } from "@/types";
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

  const rightGutter = 12;
  const width = `calc((100% - ${rightGutter}px) / ${laneCount} - 6px)`;
  const left = `calc(${laneIndex} * (100% - ${rightGutter}px) / ${laneCount} + 2px)`;

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
  const fillColor = `color-mix(in srgb, ${baseColor} 18%, white)`;
  const fillColorSelected = `color-mix(in srgb, ${baseColor} 28%, white)`;
  const endStop = entry.tags?.[0]?.tag.color ?? `color-mix(in srgb, ${baseColor} 65%, white)`;

  return (
    <div
      onPointerDown={onPointerDown}
      onClick={onSelect}
      onDoubleClick={onDoubleClick}
      className={cn(
        "absolute rounded-sm border border-transparent px-1.5 py-0.5 text-xs",
        !ghost && "cursor-grab hover:shadow-sm",
        ghost && "cursor-grabbing opacity-60",
      )}
      style={{
        top,
        height,
        left,
        width,
        ["--entry-fill" as never]: selected ? fillColorSelected : fillColor,
        background: selected
          ? `linear-gradient(color-mix(in srgb, var(--entry-fill) 80%, white), var(--entry-fill)) padding-box, conic-gradient(from var(--entry-angle), color-mix(in srgb, ${baseColor} 80%, black) 0deg, ${baseColor} 40deg, color-mix(in srgb, ${baseColor} 55%, white) 90deg, color-mix(in srgb, ${endStop} 55%, white) 130deg, ${endStop} 140deg, color-mix(in srgb, ${endStop} 80%, black) 180deg, ${endStop} 220deg, color-mix(in srgb, ${endStop} 55%, white) 230deg, color-mix(in srgb, ${baseColor} 55%, white) 270deg, ${baseColor} 320deg, color-mix(in srgb, ${baseColor} 80%, black) 360deg) border-box`
          : `linear-gradient(color-mix(in srgb, var(--entry-fill) 80%, white), var(--entry-fill)) padding-box, linear-gradient(to bottom right, ${baseColor} 0%, var(--entry-fill) 50%, ${endStop} 100%) border-box`,
        transition: "--entry-fill 220ms ease, box-shadow 150ms ease",
        ...(selected
          ? {
              animation: "entry-selected-rotate 8s linear infinite",
            }
          : {}),
        color: `color-mix(in srgb, ${baseColor} 60%, black)`,
        touchAction,
        ...style,
      }}
    >
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
        <div className="flex items-baseline gap-2">
          <div className="min-w-0 flex-1 truncate font-semibold leading-tight">
            {entry.title ? entry.title : projectName}
          </div>
          <div className="shrink-0 text-[10px] leading-tight tabular-nums opacity-60">
            {format(start, "HH:mm")}–{format(end, "HH:mm")}
          </div>
        </div>
        {entry.title && (
          <div className="flex items-baseline gap-2 leading-tight">
            <div className="min-w-0 flex-1 text-[12px] truncate opacity-75">{projectName}</div>
            <div className="ml-auto shrink-0 text-[10px] tabular-nums opacity-60">
              {durLabel}
              {breakLabel}
            </div>
          </div>
        )}
        {(!entry.title || (entry.tags && entry.tags.length > 0)) && (
          <div className="mt-0.5 flex flex-wrap items-baseline gap-x-2 gap-y-0.5 text-[10px] leading-tight opacity-70">
            {entry.tags?.map((te) => (
              <span key={te.tagId}>{te.tag.name}</span>
            ))}
            {!entry.title && (
              <span className="ml-auto tabular-nums opacity-85">
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

