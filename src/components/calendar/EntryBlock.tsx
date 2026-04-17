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
  editing = false,
  onPointerDown,
  onTopHandleDown,
  onBottomHandleDown,
  onSelect,
  onDoubleClick,
  onTitleCommit,
  onEditCancel,
  style,
}: {
  entry: TimeEntry;
  laneIndex: number;
  laneCount: number;
  selected?: boolean;
  ghost?: boolean;
  editing?: boolean;
  onPointerDown?: (e: React.PointerEvent) => void;
  onTopHandleDown?: (e: React.PointerEvent) => void;
  onBottomHandleDown?: (e: React.PointerEvent) => void;
  onSelect?: () => void;
  onDoubleClick?: () => void;
  onTitleCommit?: (title: string) => void;
  onEditCancel?: () => void;
  style?: React.CSSProperties;
}) {
  const start = new Date(entry.start);
  const end = new Date(entry.end);
  const top = minutesToY(minutesFromDayStart(start));
  const height = Math.max(minutesToY(minutesFromDayStart(end) - minutesFromDayStart(start)), 14);

  const width = `calc(${100 / laneCount}% - 2px)`;
  const left = `calc(${(laneIndex / laneCount) * 100}% + 1px)`;

  const durationMin = Math.round((end.getTime() - start.getTime()) / 60000);
  const hours = Math.floor(durationMin / 60);
  const mins = durationMin % 60;
  const durLabel = hours > 0 ? `${hours}h${mins ? ` ${mins}m` : ""}` : `${mins}m`;

  const projectName = entry.project?.name ?? "プロジェクトなし";

  return (
    <div
      onPointerDown={editing ? undefined : onPointerDown}
      onClick={onSelect}
      onDoubleClick={onDoubleClick}
      className={cn(
        "absolute rounded-md border-l-4 px-1.5 py-0.5 text-[10px] text-white shadow-sm transition-shadow",
        !ghost && !editing && "cursor-grab hover:shadow",
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
      {onTopHandleDown && !ghost && !editing && (
        <div
          onPointerDown={(e) => {
            e.stopPropagation();
            onTopHandleDown(e);
          }}
          className="absolute inset-x-0 top-0 h-1.5 cursor-ns-resize"
        />
      )}
      <div className="overflow-hidden">
        {editing ? (
          <TitleInput
            initial={entry.title ?? ""}
            onCommit={(v) => onTitleCommit?.(v)}
            onCancel={() => onEditCancel?.()}
          />
        ) : (
          <div className="truncate font-medium leading-tight">
            {entry.title ? entry.title : projectName}
          </div>
        )}
        <div className="truncate opacity-80 leading-tight">
          {entry.title ? `${projectName} · ` : ""}
          {format(start, "HH:mm")}–{format(end, "HH:mm")} · {durLabel}
        </div>
      </div>
      {onBottomHandleDown && !ghost && !editing && (
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

function TitleInput({
  initial,
  onCommit,
  onCancel,
}: {
  initial: string;
  onCommit: (value: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = React.useState(initial);
  return (
    <input
      autoFocus
      value={value}
      maxLength={100}
      onChange={(e) => setValue(e.target.value)}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      onBlur={() => onCommit(value.trim())}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          onCommit(value.trim());
        } else if (e.key === "Escape") {
          e.preventDefault();
          onCancel();
        }
      }}
      placeholder="タイトル"
      className="w-full rounded-sm bg-white/20 px-1 text-[10px] font-medium text-white placeholder-white/60 focus:bg-white/30 focus:outline-none"
    />
  );
}
