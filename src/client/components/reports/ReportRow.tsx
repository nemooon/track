import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import { format } from "date-fns";
import { apiFetch } from "@client/lib/fetcher";
import type { ReportEntriesResponse, ReportEntry, Tag } from "@shared/types";

const FALLBACK_COLORS = [
  "#3b82f6", "#ef4444", "#10b981", "#f59e0b", "#8b5cf6",
  "#ec4899", "#06b6d4", "#84cc16", "#f97316", "#6366f1",
];

function formatDuration(min: number) {
  const h = Math.floor(min / 60);
  const m = min % 60;
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

export type RowKind = "client" | "project" | "tag";

export type BaseFilters = {
  range: "week" | "month";
  anchor: string; // ISO
  clientIds: string[];
  projectIds: string[];
  tagIds: string[];
};

export type ExpansionApi = {
  expanded: Set<string>;
  toggle: (path: string) => void;
  groupSameTitles: boolean;
};

function resolveIds(base: string[], specific: string | undefined): string[] {
  return specific ? [specific] : base;
}

function appendFilters(
  params: URLSearchParams,
  base: BaseFilters,
  extra: { clientId?: string; projectId?: string; tagId?: string },
) {
  const clientIds = resolveIds(base.clientIds, extra.clientId);
  const projectIds = resolveIds(base.projectIds, extra.projectId);
  const tagIds = resolveIds(base.tagIds, extra.tagId);
  if (clientIds.length) params.set("clientIds", clientIds.join(","));
  if (projectIds.length) params.set("projectIds", projectIds.join(","));
  if (tagIds.length) params.set("tagIds", tagIds.join(","));
}

export function buildEntriesUrl(
  base: BaseFilters,
  extra: { clientId?: string; projectId?: string; tagId?: string },
): string {
  const params = new URLSearchParams({
    range: base.range,
    anchor: base.anchor,
  });
  appendFilters(params, base, extra);
  return `/api/reports/entries?${params.toString()}`;
}

function joinPath(parent: string, kind: RowKind, key: string): string {
  return parent ? `${parent}>${kind}:${key}` : `${kind}:${key}`;
}

export function ReportRow({
  rowKey,
  parentPath,
  label,
  color,
  totalMinutes,
  parentTotal,
  kind,
  base,
  ancestor,
  depth,
  fallbackColorIndex,
  expansion,
}: {
  rowKey: string;
  parentPath: string;
  label: string;
  color?: string;
  totalMinutes: number;
  parentTotal: number;
  kind: RowKind;
  base: BaseFilters;
  ancestor: { clientId?: string; projectId?: string; tagId?: string };
  depth: number;
  fallbackColorIndex: number;
  expansion: ExpansionApi;
}) {
  const path = joinPath(parentPath, kind, rowKey);
  const open = expansion.expanded.has(path);
  const displayColor = color ?? FALLBACK_COLORS[fallbackColorIndex % FALLBACK_COLORS.length];
  const percent = parentTotal > 0 ? Math.round((totalMinutes / parentTotal) * 100) : 0;

  const childAncestor = {
    ...ancestor,
    ...(kind === "client" ? { clientId: rowKey } : {}),
    ...(kind === "tag" ? { tagId: rowKey } : {}),
    ...(kind === "project" ? { projectId: rowKey } : {}),
  };

  return (
    <>
      <tr className="border-b border-neutral-100">
        <td className="py-2" style={{ paddingLeft: 8 + depth * 20 }}>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => expansion.toggle(path)}
              className="flex h-4 w-4 shrink-0 items-center justify-center rounded text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700"
              aria-label={open ? "折りたたむ" : "展開"}
            >
              <svg
                width="8"
                height="8"
                viewBox="0 0 8 8"
                fill="none"
                style={{ transform: open ? "rotate(90deg)" : "none", transition: "transform 0.1s" }}
              >
                <path d="M2 1l3 3-3 3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
            <span
              className="inline-block h-3 w-3 shrink-0 rounded-sm"
              style={{ background: displayColor }}
            />
            <span className="truncate">{label}</span>
          </div>
        </td>
        <td className="py-2 text-right tabular-nums">{formatDuration(totalMinutes)}</td>
        <td className="py-2 text-right tabular-nums text-neutral-500">{percent}%</td>
      </tr>
      {open && (
        <ChildEntries
          base={base}
          ancestor={childAncestor}
          parentTotal={totalMinutes}
          depth={depth + 1}
          groupSameTitles={expansion.groupSameTitles}
        />
      )}
    </>
  );
}

type GroupedEntry = {
  key: string;
  title: string | null;
  minutes: number;
  count: number;
  project: ReportEntry["project"];
  tags: Tag[];
};

export function groupEntriesByTitle(entries: ReportEntry[]): GroupedEntry[] {
  const map = new Map<string, GroupedEntry>();
  for (const e of entries) {
    const key = (e.title ?? "").trim() || "__untitled__";
    const existing = map.get(key);
    if (existing) {
      existing.minutes += e.minutes;
      existing.count += 1;
      // Merge tags by id
      for (const t of e.tags) {
        if (!existing.tags.some((x) => x.id === t.id)) existing.tags.push(t);
      }
      // If projects differ, drop the project so we don't mislead
      if (existing.project && e.project && existing.project.id !== e.project.id) {
        existing.project = null;
      } else if (!existing.project) {
        // already null, leave it
      }
    } else {
      map.set(key, {
        key,
        title: e.title,
        minutes: e.minutes,
        count: 1,
        project: e.project,
        tags: [...e.tags],
      });
    }
  }
  return Array.from(map.values()).sort((a, b) => b.minutes - a.minutes);
}

function ChildEntries({
  base,
  ancestor,
  parentTotal,
  depth,
  groupSameTitles,
}: {
  base: BaseFilters;
  ancestor: { clientId?: string; projectId?: string; tagId?: string };
  parentTotal: number;
  depth: number;
  groupSameTitles: boolean;
}) {
  const url = buildEntriesUrl(base, ancestor);
  const { data, isLoading } = useQuery({
    queryKey: ["reports-entries", url],
    queryFn: () => apiFetch<ReportEntriesResponse>(url),
  });

  if (isLoading) {
    return (
      <tr>
        <td colSpan={3} style={{ paddingLeft: 8 + depth * 20 }} className="py-2">
          <span className="text-xs text-neutral-400">読み込み中…</span>
        </td>
      </tr>
    );
  }

  const entries = data?.entries ?? [];
  if (entries.length === 0) {
    return (
      <tr>
        <td colSpan={3} style={{ paddingLeft: 8 + depth * 20 }} className="py-2">
          <span className="text-xs text-neutral-400">エントリがありません</span>
        </td>
      </tr>
    );
  }

  if (groupSameTitles) {
    const groups = groupEntriesByTitle(entries);
    return (
      <>
        {groups.map((g) => {
          const percent = parentTotal > 0 ? Math.round((g.minutes / parentTotal) * 100) : 0;
          return (
            <tr key={g.key} className="border-b border-neutral-100 text-neutral-700">
              <td className="py-1.5" style={{ paddingLeft: 8 + depth * 20 }}>
                <div className="flex items-center gap-2">
                  <span className="h-4 w-4 shrink-0" />
                  {g.project ? (
                    <span
                      className="inline-block h-2 w-2 shrink-0 rounded-full"
                      style={{ background: g.project.color }}
                    />
                  ) : (
                    <span className="inline-block h-2 w-2 shrink-0 rounded-full bg-neutral-300" />
                  )}
                  <span className="truncate">
                    {g.title || <span className="text-neutral-400">（タイトルなし）</span>}
                  </span>
                  <span className="text-[10px] text-neutral-400">×{g.count}</span>
                  {g.tags.length > 0 && (
                    <span className="flex gap-1">
                      {g.tags.map((t) => (
                        <span
                          key={t.id}
                          className="rounded-full px-1.5 py-px text-[10px] text-white"
                          style={{ background: t.color }}
                        >
                          {t.name}
                        </span>
                      ))}
                    </span>
                  )}
                </div>
              </td>
              <td className="py-1.5 text-right tabular-nums">{formatDuration(g.minutes)}</td>
              <td className="py-1.5 text-right tabular-nums text-neutral-400">{percent}%</td>
            </tr>
          );
        })}
      </>
    );
  }

  return (
    <>
      {entries.map((entry) => {
        const start = new Date(entry.start);
        const end = new Date(entry.end);
        const dateLabel = format(start, "M/d (E) HH:mm");
        const endLabel = format(end, "HH:mm");
        const percent = parentTotal > 0 ? Math.round((entry.minutes / parentTotal) * 100) : 0;
        return (
          <tr key={entry.id} className="border-b border-neutral-100 text-neutral-700">
            <td className="py-1.5" style={{ paddingLeft: 8 + depth * 20 }}>
              <div className="flex items-center gap-2">
                <span className="h-4 w-4 shrink-0" />
                <span
                  className="inline-block h-2 w-2 shrink-0 rounded-full"
                  style={{ background: entry.project?.color ?? "#d4d4d4" }}
                />
                <span className="text-xs text-neutral-500 tabular-nums">
                  {dateLabel}–{endLabel}
                </span>
                <span className="truncate">
                  {entry.title || (
                    <span className="text-neutral-400">（タイトルなし）</span>
                  )}
                </span>
                {entry.tags.length > 0 && (
                  <span className="flex gap-1">
                    {entry.tags.map((t) => (
                      <span
                        key={t.id}
                        className="rounded-full px-1.5 py-px text-[10px] text-white"
                        style={{ background: t.color }}
                      >
                        {t.name}
                      </span>
                    ))}
                  </span>
                )}
              </div>
            </td>
            <td className="py-1.5 text-right tabular-nums">{formatDuration(entry.minutes)}</td>
            <td className="py-1.5 text-right tabular-nums text-neutral-400">{percent}%</td>
          </tr>
        );
      })}
    </>
  );
}
