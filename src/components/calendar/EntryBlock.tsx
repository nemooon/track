"use client";

import * as React from "react";
import { format } from "date-fns";
import { cn } from "@/lib/utils";
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
}) {
  const start = new Date(entry.start);
  const end = new Date(entry.end);
  const top = minutesToY(minutesFromDayStart(start), hourPx) + 1;
  const height = Math.max(minutesToY(minutesFromDayStart(end) - minutesFromDayStart(start), hourPx) - 3, 14);

  const width = `calc(${100 / laneCount}% - 6px)`;
  const left = `calc(${(laneIndex / laneCount) * 100}% + 3px)`;

  const durationMin = Math.round((end.getTime() - start.getTime()) / 60000);
  const hours = Math.floor(durationMin / 60);
  const mins = durationMin % 60;
  const durLabel = hours > 0 ? `${hours}h${mins ? ` ${mins}m` : ""}` : `${mins}m`;

  const projectName = entry.project?.name ?? "プロジェクトなし";

  return (
    <div
      onPointerDown={onPointerDown}
      onClick={onSelect}
      onDoubleClick={onDoubleClick}
      className={cn(
        "absolute rounded-md border-l-4 px-1.5 py-0.5 text-xs text-white shadow-sm transition-shadow",
        !ghost && "cursor-grab hover:shadow",
        ghost && "cursor-grabbing opacity-60",
        selected && "ring-2 ring-neutral-900 ring-offset-1",
      )}
      style={{
        top,
        height,
        left,
        width,
        backgroundColor: entry.project?.color ?? "#9ca3af",
        borderLeftColor: `color-mix(in srgb, ${entry.project?.color ?? "#9ca3af"} 70%, black)`,
        touchAction: "none",
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
      <div className="overflow-hidden">
        <div className="truncate font-medium leading-tight">
          {entry.title ? entry.title : projectName}
        </div>
        <div className="truncate opacity-80 leading-tight">
          {entry.title ? `${projectName} · ` : ""}
          {format(start, "HH:mm")}–{format(end, "HH:mm")} · {durLabel}
        </div>
        {entry.tags && entry.tags.length > 0 && (
          <div className="mt-0.5 flex flex-wrap gap-0.5">
            {entry.tags.map((te) => (
              <span
                key={te.tagId}
                className="inline-block rounded-sm bg-white/25 px-1 text-[10px] leading-tight"
              >
                {te.tag.name}
              </span>
            ))}
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

