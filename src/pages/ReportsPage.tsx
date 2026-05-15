import { useCallback, useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell } from "recharts";
import { addDays, addMonths } from "date-fns";
import { Button } from "@/components/ui/button";
import { apiFetch } from "@/lib/fetcher";
import { formatWeekLabel, formatMonthLabel } from "@/lib/time";
import { FilterMultiSelect, type FilterOption } from "@/components/reports/FilterMultiSelect";
import {
  ReportRow,
  type BaseFilters,
  type ExpansionApi,
  type RowKind,
} from "@/components/reports/ReportRow";
import type { Client, Project, Tag, ReportResponse } from "@/types";

const COLORS = [
  "#3b82f6", "#ef4444", "#10b981", "#f59e0b", "#8b5cf6",
  "#ec4899", "#06b6d4", "#84cc16", "#f97316", "#6366f1",
];

function formatDuration(min: number) {
  const h = Math.floor(min / 60);
  const m = min % 60;
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

export function ReportsPage() {
  const [range, setRange] = useState<"week" | "month">("week");
  const [anchor, setAnchor] = useState(() => new Date());
  const [groupBy, setGroupBy] = useState<RowKind>("project");
  const [selectedClientIds, setSelectedClientIds] = useState<string[]>([]);
  const [selectedProjectIds, setSelectedProjectIds] = useState<string[]>([]);
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>([]);

  const { data: clients = [] } = useQuery({
    queryKey: ["clients"],
    queryFn: () => apiFetch<Client[]>("/api/clients"),
  });
  const { data: projects = [] } = useQuery({
    queryKey: ["projects"],
    queryFn: () => apiFetch<Project[]>("/api/projects"),
  });
  const { data: tagList = [] } = useQuery({
    queryKey: ["tags"],
    queryFn: () => apiFetch<Tag[]>("/api/tags"),
  });

  const clientOptions: FilterOption[] = useMemo(
    () =>
      clients
        .filter((c) => !c.archived)
        .map((c) => ({ id: c.id, label: c.name })),
    [clients],
  );

  const projectOptions: FilterOption[] = useMemo(() => {
    const visible = projects
      .filter((p) => !p.archived)
      .filter((p) => selectedClientIds.length === 0 || selectedClientIds.includes(p.clientId));
    return visible.map((p) => ({
      id: p.id,
      label: p.name,
      hint: p.client.name,
      color: p.color,
    }));
  }, [projects, selectedClientIds]);

  const tagOptions: FilterOption[] = useMemo(
    () => tagList.map((t) => ({ id: t.id, label: t.name, color: t.color })),
    [tagList],
  );

  const anchorIso = anchor.toISOString();

  const baseFilters: BaseFilters = useMemo(
    () => ({
      range,
      anchor: anchorIso,
      clientIds: selectedClientIds,
      projectIds: selectedProjectIds,
      tagIds: selectedTagIds,
    }),
    [range, anchorIso, selectedClientIds, selectedProjectIds, selectedTagIds],
  );

  const reportsUrl = useMemo(() => {
    const p = new URLSearchParams({ range, anchor: anchorIso, groupBy });
    if (selectedClientIds.length) p.set("clientIds", selectedClientIds.join(","));
    if (selectedProjectIds.length) p.set("projectIds", selectedProjectIds.join(","));
    if (selectedTagIds.length) p.set("tagIds", selectedTagIds.join(","));
    return `/api/reports?${p.toString()}`;
  }, [range, anchorIso, groupBy, selectedClientIds, selectedProjectIds, selectedTagIds]);

  const { data, isLoading } = useQuery({
    queryKey: ["reports", reportsUrl],
    queryFn: () => apiFetch<ReportResponse>(reportsUrl),
  });

  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [groupSameTitles, setGroupSameTitles] = useState(false);
  const [expandAllMode, setExpandAllMode] = useState(false);

  // groupBy が変わると行の path 体系が変わるのでリセット
  useEffect(() => {
    setExpandedPaths(new Set());
    setExpandAllMode(false);
  }, [groupBy]);

  // すべて展開モード中は、データ更新時に最上位の行をすべて展開する
  useEffect(() => {
    if (!expandAllMode || !data) return;
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      for (const r of data.rows) {
        next.add(`${groupBy}:${r.key}`);
      }
      return next;
    });
  }, [data, expandAllMode, groupBy]);

  const togglePath = useCallback((path: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  const expansion: ExpansionApi = useMemo(
    () => ({ expanded: expandedPaths, toggle: togglePath, groupSameTitles }),
    [expandedPaths, togglePath, groupSameTitles],
  );

  const anyExpanded = expandedPaths.size > 0;
  function toggleAll() {
    if (anyExpanded) {
      setExpandedPaths(new Set());
      setExpandAllMode(false);
    } else {
      const initial = new Set<string>();
      for (const r of rows) {
        initial.add(`${groupBy}:${r.key}`);
      }
      setExpandedPaths(initial);
      setExpandAllMode(true);
    }
  }

  function prev() {
    setAnchor((a) => (range === "week" ? addDays(a, -7) : addMonths(a, -1)));
  }
  function next() {
    setAnchor((a) => (range === "week" ? addDays(a, 7) : addMonths(a, 1)));
  }
  function today() {
    setAnchor(new Date());
  }

  const label = range === "week" ? formatWeekLabel(anchor) : formatMonthLabel(anchor);

  const rows = data?.rows ?? [];
  const total = data?.totalMinutes ?? 0;

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-4 sm:p-6">
      {/* Range / GroupBy / Navigation */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex gap-1 rounded-md border border-neutral-200 p-0.5">
          {(["week", "month"] as const).map((r) => (
            <button
              key={r}
              onClick={() => setRange(r)}
              className={`rounded px-3 py-1 text-sm ${range === r ? "bg-neutral-900 text-white" : "text-neutral-600 hover:bg-neutral-100"}`}
            >
              {r === "week" ? "週" : "月"}
            </button>
          ))}
        </div>
        <div className="flex gap-1 rounded-md border border-neutral-200 p-0.5">
          {(["client", "project", "tag"] as const).map((g) => (
            <button
              key={g}
              onClick={() => setGroupBy(g)}
              className={`rounded px-3 py-1 text-sm ${groupBy === g ? "bg-neutral-900 text-white" : "text-neutral-600 hover:bg-neutral-100"}`}
            >
              {g === "client" ? "クライアント" : g === "project" ? "プロジェクト" : "タグ"}
            </button>
          ))}
        </div>

        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={prev}>
            ←
          </Button>
          <Button variant="outline" size="sm" onClick={today}>
            今日
          </Button>
          <Button variant="outline" size="sm" onClick={next}>
            →
          </Button>
          <span className="text-sm font-medium">{label}</span>
        </div>
      </div>

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-xs text-neutral-500">絞り込み:</span>
        <FilterMultiSelect
          label="クライアント"
          options={clientOptions}
          selected={selectedClientIds}
          onChange={(next) => {
            setSelectedClientIds(next);
            // Drop project selections that no longer match the client filter
            if (next.length > 0) {
              setSelectedProjectIds((prev) =>
                prev.filter((pid) => {
                  const proj = projects.find((p) => p.id === pid);
                  return proj ? next.includes(proj.clientId) : false;
                }),
              );
            }
          }}
        />
        <FilterMultiSelect
          label="プロジェクト"
          options={projectOptions}
          selected={selectedProjectIds}
          onChange={setSelectedProjectIds}
        />
        <FilterMultiSelect
          label="タグ"
          options={tagOptions}
          selected={selectedTagIds}
          onChange={setSelectedTagIds}
        />
        {(selectedClientIds.length > 0 ||
          selectedProjectIds.length > 0 ||
          selectedTagIds.length > 0) && (
          <button
            type="button"
            onClick={() => {
              setSelectedClientIds([]);
              setSelectedProjectIds([]);
              setSelectedTagIds([]);
            }}
            className="text-xs text-neutral-500 underline-offset-2 hover:text-neutral-900 hover:underline"
          >
            すべてクリア
          </button>
        )}
      </div>

      {/* Chart */}
      {isLoading ? (
        <div className="flex h-48 items-center justify-center">
          <div className="h-6 w-6 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-700" />
        </div>
      ) : rows.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-neutral-400">
          この期間にデータがありません
        </div>
      ) : (
        <ResponsiveContainer width="100%" height={300}>
          <BarChart data={rows} layout="vertical" margin={{ left: 120 }}>
            <XAxis type="number" tickFormatter={(v) => formatDuration(v)} />
            <YAxis type="category" dataKey="label" width={120} tick={{ fontSize: 12 }} />
            <Tooltip formatter={(value) => formatDuration(value as number)} />
            <Bar dataKey="totalMinutes" radius={[0, 4, 4, 0]}>
              {rows.map((row, i) => (
                <Cell key={row.key} fill={row.color || COLORS[i % COLORS.length]} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      )}

      {/* Table */}
      {rows.length > 0 && (
        <div className="space-y-2">
          <div className="flex items-center justify-end gap-4">
            <label className="flex cursor-pointer items-center gap-2 text-xs text-neutral-600">
              <span>同名エントリをまとめる</span>
              <span
                role="switch"
                aria-checked={groupSameTitles}
                tabIndex={0}
                onClick={() => setGroupSameTitles((v) => !v)}
                onKeyDown={(e) => {
                  if (e.key === " " || e.key === "Enter") {
                    e.preventDefault();
                    setGroupSameTitles((v) => !v);
                  }
                }}
                className={`relative inline-flex h-4 w-7 shrink-0 items-center rounded-full transition-colors ${
                  groupSameTitles ? "bg-neutral-900" : "bg-neutral-300"
                }`}
              >
                <span
                  className={`inline-block h-3 w-3 transform rounded-full bg-white shadow transition-transform ${
                    groupSameTitles ? "translate-x-3.5" : "translate-x-0.5"
                  }`}
                />
              </span>
            </label>
            <button
              type="button"
              onClick={toggleAll}
              className="text-xs text-neutral-500 underline-offset-2 hover:text-neutral-900 hover:underline"
            >
              {anyExpanded ? "すべて折りたたむ" : "すべて展開"}
            </button>
          </div>
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-neutral-200 text-left text-neutral-500">
                <th className="py-2 font-medium">ラベル</th>
                <th className="py-2 w-18 text-right font-medium">時間</th>
                <th className="py-2 w-18 text-right font-medium">割合</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row, i) => (
                <ReportRow
                  key={row.key}
                  rowKey={row.key}
                  parentPath=""
                  label={row.label}
                  color={row.color}
                  totalMinutes={row.totalMinutes}
                  parentTotal={total}
                  kind={groupBy}
                  base={baseFilters}
                  ancestor={{}}
                  depth={0}
                  fallbackColorIndex={i}
                  expansion={expansion}
                />
              ))}
              <tr className="font-medium">
                <td className="py-2 pl-2">合計</td>
                <td className="py-2 text-right tabular-nums">{formatDuration(total)}</td>
                <td className="py-2 text-right tabular-nums">100%</td>
              </tr>
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
