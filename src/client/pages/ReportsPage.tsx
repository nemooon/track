import { useCallback, useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useSearchParams } from "react-router";
import {
  PieChart,
  Pie,
  Tooltip,
  ResponsiveContainer,
  Cell,
} from "recharts";
import {
  addDays,
  addMonths,
  endOfMonth,
  format,
  startOfMonth,
  startOfWeek,
} from "date-fns";
import { Check, Copy } from "lucide-react";
import { apiFetch } from "@client/lib/fetcher";
import { ToolbarDateNavigation } from "@client/components/ToolbarDateNavigation";
import { ViewToolbar } from "@client/components/ViewToolbar";
import {
  ToolbarControlButton,
  ToolbarControlGroup,
} from "@client/components/ToolbarControls";
import { FilterMultiSelect, type FilterOption } from "@client/components/reports/FilterMultiSelect";
import {
  ReportRow,
  buildEntriesUrl,
  groupEntriesByTitle,
  type BaseFilters,
  type ExpansionApi,
  type RowKind,
} from "@client/components/reports/ReportRow";
import type {
  Client,
  Project,
  Tag,
  ReportResponse,
  ReportEntriesResponse,
} from "@shared/types";

const COLORS = [
  "#3b82f6", "#ef4444", "#10b981", "#f59e0b", "#8b5cf6",
  "#ec4899", "#06b6d4", "#84cc16", "#f97316", "#6366f1",
];

function formatDuration(min: number) {
  const h = Math.floor(min / 60);
  const m = min % 60;
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

// --- Slack 貼り付け用テキスト表の生成 -----------------------------------------
// Slack はタブ区切り（TSV）テキストを貼り付けると表として認識・添付できる。

function periodLabel(anchor: Date, range: "week" | "month"): string {
  if (range === "month") return format(anchor, "yyyy年M月");
  const start = startOfWeek(anchor, { weekStartsOn: 0 });
  const end = addDays(start, 6);
  const sameMonth =
    start.getFullYear() === end.getFullYear() &&
    start.getMonth() === end.getMonth();
  return sameMonth
    ? `${format(start, "yyyy/M/d")} – ${format(end, "d")}`
    : `${format(start, "yyyy/M/d")} – ${format(end, "M/d")}`;
}

type SlackGroup = {
  label: string;
  totalMinutes: number;
  entries: { title: string; minutes: number }[];
};

function buildSlackTable(opts: {
  groups: SlackGroup[];
  total: number;
  groupBy: RowKind;
  periodText: string;
}): string {
  const { groups, total, groupBy, periodText } = opts;
  const groupLabel =
    groupBy === "client" ? "クライアント" : groupBy === "project" ? "プロジェクト" : "タグ";

  const pct = (min: number) =>
    total > 0 ? `${Math.round((min / total) * 100)}%` : "0%";

  // 1 行目（ヘッダー）に期間を含め、貼り付けた表だけで文脈が分かるようにする
  const lines: string[][] = [[`${groupLabel}（${periodText}）`, "時間", "割合"]];
  for (const g of groups) {
    lines.push([g.label, formatDuration(g.totalMinutes), pct(g.totalMinutes)]);
    for (const e of g.entries) {
      lines.push([`└ ${e.title || "（タイトルなし）"}`, formatDuration(e.minutes), ""]);
    }
  }
  lines.push(["合計", formatDuration(total), "100%"]);

  return lines.map((cells) => cells.join("\t")).join("\n");
}

type TooltipPayload = {
  value: number;
  payload: { label: string; color?: string };
};

function ChartTooltip({
  active,
  payload,
  stripClient,
}: {
  active?: boolean;
  payload?: TooltipPayload[];
  stripClient?: boolean;
}) {
  if (!active || !payload || !payload.length) return null;
  const item = payload[0];
  const color = item.payload.color;
  const label = stripClient
    ? item.payload.label.split(" · ").slice(-1)[0]
    : item.payload.label;
  return (
    <div className="rounded-lg border border-neutral-200 bg-white/90 px-3 py-2 text-xs shadow-lg backdrop-blur">
      <div className="flex items-center gap-1.5 font-medium text-neutral-900">
        {color && (
          <span
            className="inline-block h-2 w-2 rounded-full"
            style={{ background: color }}
          />
        )}
        {label}
      </div>
      <div className="mt-0.5 tabular-nums text-neutral-600">
        {formatDuration(item.value)}
      </div>
    </div>
  );
}

// "yyyy-MM-dd"（ローカル＝JST の暦日）をローカル 0:00 の Date に戻す
function parseLocalDate(s: string | null): Date | null {
  if (!s) return null;
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return Number.isNaN(d.getTime()) ? null : d;
}

function parseIdList(s: string | null): string[] {
  return s ? s.split(",").filter(Boolean) : [];
}

// 折りたたみ状態・同名まとめは localStorage で永続化する
const EXPANDED_KEY = "reports.expandedPaths";
const SAME_TITLES_KEY = "reports.groupSameTitles";

// 折りたたみは groupBy ごとに path 体系が異なるため、グループ単位で保存する
function readExpandedStore(): Record<string, string[]> {
  try {
    const raw = localStorage.getItem(EXPANDED_KEY);
    const v = raw ? JSON.parse(raw) : null;
    return v && typeof v === "object" ? (v as Record<string, string[]>) : {};
  } catch {
    return {};
  }
}

export function ReportsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [range, setRange] = useState<"week" | "month">(() =>
    searchParams.get("range") === "month" ? "month" : "week",
  );
  const [anchor, setAnchor] = useState(
    () => parseLocalDate(searchParams.get("anchor")) ?? new Date(),
  );
  const [groupBy, setGroupBy] = useState<RowKind>(() => {
    const g = searchParams.get("groupBy");
    return g === "client" || g === "tag" ? g : "project";
  });
  const [selectedClientIds, setSelectedClientIds] = useState<string[]>(() =>
    parseIdList(searchParams.get("clientIds")),
  );
  const [selectedProjectIds, setSelectedProjectIds] = useState<string[]>(() =>
    parseIdList(searchParams.get("projectIds")),
  );
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>(() =>
    parseIdList(searchParams.get("tagIds")),
  );

  // 表示状態を URL クエリに同期（リロード・共有で同じビューを復元）
  useEffect(() => {
    const p = new URLSearchParams();
    if (range !== "week") p.set("range", range);
    p.set("anchor", format(anchor, "yyyy-MM-dd"));
    if (groupBy !== "project") p.set("groupBy", groupBy);
    if (selectedClientIds.length) p.set("clientIds", selectedClientIds.join(","));
    if (selectedProjectIds.length) p.set("projectIds", selectedProjectIds.join(","));
    if (selectedTagIds.length) p.set("tagIds", selectedTagIds.join(","));
    setSearchParams(p, { replace: true });
  }, [
    range,
    anchor,
    groupBy,
    selectedClientIds,
    selectedProjectIds,
    selectedTagIds,
    setSearchParams,
  ]);

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

  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(
    () => new Set(readExpandedStore()[groupBy] ?? []),
  );
  const [groupSameTitles, setGroupSameTitles] = useState(
    () => localStorage.getItem(SAME_TITLES_KEY) === "1",
  );
  const [expandAllMode, setExpandAllMode] = useState(false);

  // groupBy が変わったら、その groupBy 用に保存済みの折りたたみ状態を読み込む
  useEffect(() => {
    setExpandedPaths(new Set(readExpandedStore()[groupBy] ?? []));
    setExpandAllMode(false);
  }, [groupBy]);

  // 折りたたみ状態を groupBy ごとに保存
  useEffect(() => {
    const prefix = `${groupBy}:`;
    // groupBy 切替直後は expandedPaths がまだ旧 groupBy のものなので保存しない
    if (![...expandedPaths].every((p) => p.startsWith(prefix))) return;
    const store = readExpandedStore();
    store[groupBy] = [...expandedPaths];
    try {
      localStorage.setItem(EXPANDED_KEY, JSON.stringify(store));
    } catch {
      // 保存失敗は無視（プライベートモード等）
    }
  }, [expandedPaths, groupBy]);

  // 同名まとめトグルを保存
  useEffect(() => {
    localStorage.setItem(SAME_TITLES_KEY, groupSameTitles ? "1" : "0");
  }, [groupSameTitles]);

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

  const rows = data?.rows ?? [];
  const total = data?.totalMinutes ?? 0;

  const queryClient = useQueryClient();
  const [copied, setCopied] = useState(false);
  const [copying, setCopying] = useState(false);
  async function copyTable() {
    setCopying(true);
    let text: string;
    try {
      const groups = await Promise.all(
        rows.map(async (r) => {
          const label =
            groupBy === "project" ? r.label.split(" · ").slice(-1)[0] : r.label;
          // 画面どおり：展開中の行だけ中身を含め、折りたたみ中は集計行のみ
          const isExpanded = expandedPaths.has(`${groupBy}:${r.key}`);
          if (!isExpanded) {
            return { label, totalMinutes: r.totalMinutes, entries: [] };
          }
          const extra =
            groupBy === "client"
              ? { clientId: r.key }
              : groupBy === "project"
                ? { projectId: r.key }
                : { tagId: r.key };
          const url = buildEntriesUrl(baseFilters, extra);
          const res = await queryClient.fetchQuery({
            queryKey: ["reports-entries", url],
            queryFn: () => apiFetch<ReportEntriesResponse>(url),
          });
          const raw = res?.entries ?? [];
          const entries = groupSameTitles
            ? groupEntriesByTitle(raw).map((g) => ({
                title: g.title ?? "",
                minutes: g.minutes,
              }))
            : raw.map((e) => ({ title: e.title ?? "", minutes: e.minutes }));
          return { label, totalMinutes: r.totalMinutes, entries };
        }),
      );
      text = buildSlackTable({
        groups,
        total,
        groupBy,
        periodText: periodLabel(anchor, range),
      });
    } finally {
      setCopying(false);
    }
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // クリップボード API 非対応時のフォールバック
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }

  return (
    <div className="min-h-full">
      <ViewToolbar>
        <ToolbarDateNavigation
          anchor={anchor}
          range={range === "week" ? { kind: "week" } : { kind: "month" }}
          onPrev={prev}
          onNext={next}
          onAnchorChange={setAnchor}
          onToday={() => setAnchor(new Date())}
        />
        <ToolbarControlGroup>
          {(["week", "month"] as const).map((r) => (
            <ToolbarControlButton
              key={r}
              onClick={() => setRange(r)}
              active={range === r}
              aria-pressed={range === r}
            >
              {r === "week" ? "週" : "月"}
            </ToolbarControlButton>
          ))}
        </ToolbarControlGroup>
        <ToolbarControlGroup>
          {(["client", "project", "tag"] as const).map((g) => (
            <ToolbarControlButton
              key={g}
              onClick={() => setGroupBy(g)}
              active={groupBy === g}
              aria-pressed={groupBy === g}
            >
              {g === "client" ? "クライアント" : g === "project" ? "プロジェクト" : "タグ"}
            </ToolbarControlButton>
          ))}
        </ToolbarControlGroup>
      </ViewToolbar>

      <div className="mx-auto max-w-4xl space-y-6 p-4 sm:p-6">
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
        <div className="rounded-xl border border-neutral-100 p-4 sm:p-6">
          <div className="flex flex-col items-center gap-6 sm:flex-row sm:items-stretch">
            <div className="relative h-72 w-72 shrink-0 sm:h-80 sm:w-80">
              <ResponsiveContainer width="100%" height="100%">
                <PieChart>
                  <defs>
                    {rows.map((row, i) => {
                      const color = row.color || COLORS[i % COLORS.length];
                      return (
                        <linearGradient
                          key={i}
                          id={`donut-grad-${i}`}
                          x1="0"
                          y1="0"
                          x2="1"
                          y2="1"
                        >
                          <stop offset="0%" stopColor={color} stopOpacity={0.85} />
                          <stop offset="100%" stopColor={color} stopOpacity={1} />
                        </linearGradient>
                      );
                    })}
                  </defs>
                  <Pie
                    data={rows}
                    dataKey="totalMinutes"
                    nameKey="label"
                    innerRadius="62%"
                    outerRadius="95%"
                    paddingAngle={rows.length > 1 ? 2 : 0}
                    cornerRadius={4}
                    stroke="#ffffff"
                    strokeWidth={2}
                    isAnimationActive={false}
                  >
                    {rows.map((row, i) => (
                      <Cell key={row.key} fill={`url(#donut-grad-${i})`} />
                    ))}
                  </Pie>
                  <Tooltip
                    content={<ChartTooltip stripClient={groupBy === "project"} />}
                  />
                </PieChart>
              </ResponsiveContainer>
              <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
                <div className="text-[10px] uppercase tracking-wider text-neutral-400">
                  Total
                </div>
                <div className="mt-0.5 text-2xl font-semibold tabular-nums text-neutral-900">
                  {formatDuration(total)}
                </div>
              </div>
            </div>
            <ul className="flex-1 self-center space-y-2.5 text-sm">
              {rows.map((row, i) => {
                const color = row.color || COLORS[i % COLORS.length];
                const displayLabel =
                  groupBy === "project"
                    ? row.label.split(" · ").slice(-1)[0]
                    : row.label;
                const pct = total > 0 ? (row.totalMinutes / total) * 100 : 0;
                return (
                  <li key={row.key} className="space-y-1">
                    <div className="flex items-center gap-2">
                      <span
                        className="inline-block h-2.5 w-2.5 shrink-0 rounded-full"
                        style={{ background: color }}
                      />
                      <span className="truncate text-neutral-700">
                        {displayLabel}
                      </span>
                    </div>
                    <div className="ml-[18px] h-1.5 overflow-hidden rounded-full bg-neutral-100">
                      <div
                        className="h-full rounded-full"
                        style={{
                          width: `${pct}%`,
                          background: `linear-gradient(90deg, ${color}b3, ${color})`,
                        }}
                      />
                    </div>
                  </li>
                );
              })}
            </ul>
          </div>
        </div>
      )}

      {/* Table */}
      {rows.length > 0 && (
        <div className="space-y-2">
          <div className="flex items-center justify-end gap-4">
            <button
              type="button"
              onClick={copyTable}
              disabled={copying}
              className="inline-flex items-center gap-1.5 rounded-md border border-neutral-200 px-2.5 py-1 text-xs text-neutral-600 hover:bg-neutral-50 disabled:opacity-60"
            >
              {copied ? (
                <>
                  <Check className="size-3.5 text-emerald-600" />
                  コピーしました
                </>
              ) : (
                <>
                  <Copy className="size-3.5" />
                  {copying ? "取得中…" : "Slack用にコピー"}
                </>
              )}
            </button>
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
    </div>
  );
}
