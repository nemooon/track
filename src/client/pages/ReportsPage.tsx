import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
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
import {
  ChartPie,
  Check,
  ChevronsDownUp,
  ChevronsUpDown,
  Copy,
  FileOutput,
  LoaderCircle,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { apiFetch } from "@client/lib/fetcher";
import {
  aiProviderLabel,
  generateAiText,
  generateAiTextStream,
} from "@client/lib/ai";
import { Button } from "@client/components/ui/button";
import { Select } from "@client/components/ui/select";
import { MarkdownEditor } from "@client/components/MarkdownEditor";
import {
  Dialog,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@client/components/ui/dialog";
import { ToolbarDateNavigation } from "@client/components/ToolbarDateNavigation";
import { ViewToolbar } from "@client/components/ViewToolbar";
import {
  ToolbarControlButton,
  ToolbarControlGroup,
} from "@client/components/ToolbarControls";
import { FilterMultiSelect, type FilterOption } from "@client/components/reports/FilterMultiSelect";
import {
  aggregationColumnValue,
  buildAggregationCopyText,
  reportCopyFormatUsesAi,
  reportCopyColumnHeader,
} from "@client/lib/reportCopy";
import {
  aggregateAiResult,
  buildReportAggregationPrompt,
  type AiAggregationRow,
} from "@client/lib/reportAggregation";
import {
  buildReportOutputCacheKey,
  readReportOutputCache,
  writeReportOutputCache,
} from "@client/lib/reportOutputCache";
import {
  ReportRow,
  buildEntriesUrl,
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
  UserSettings,
  AppConfig,
  AiProgressUpdate,
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

function formatJapaneseDuration(min: number) {
  const h = Math.floor(min / 60);
  const m = min % 60;
  if (h === 0) return `${m}分`;
  return m > 0 ? `${h}時間${m}分` : `${h}時間`;
}

function formatElapsedTime(seconds: number) {
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return `${minutes}:${String(rest).padStart(2, "0")}`;
}

function ReportTableColumns() {
  return (
    <colgroup>
      <col />
      <col className="w-40" />
      <col className="w-28" />
      <col className="w-24" />
      <col className="w-16" />
    </colgroup>
  );
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

function buildWeeklyReportPrompt(opts: {
  template: string;
  period: string;
  data: ReportEntriesResponse;
}): { prompt: string; mainWork: string } {
  const { period, data } = opts;
  const template = opts.template
    .replaceAll("{{期間}}", period)
    .replaceAll("{{合計時間}}", formatJapaneseDuration(data.totalMinutes));
  const projectTotals = new Map<
    string,
    {
      label: string;
      minutes: number;
      count: number;
      facts: Map<string, { title: string; note: string }>;
    }
  >();
  for (const entry of data.entries) {
    const clientName = entry.project?.client.name ?? "クライアントなし";
    const project = entry.project
      ? `${clientName} / ${entry.project.name}`
      : "プロジェクトなし";
    const title = entry.title?.trim() || "タイトルなし";
    const note = entry.note?.replace(/\s+/g, " ").trim() || "";
    const factKey = JSON.stringify([title, note]);
    const projectKey = entry.project?.id ?? "__no_project__";
    const projectTotal = projectTotals.get(projectKey);
    if (projectTotal) {
      projectTotal.minutes += entry.minutes;
      projectTotal.count += 1;
      projectTotal.facts.set(factKey, { title, note });
    } else {
      projectTotals.set(projectKey, {
        label: project,
        minutes: entry.minutes,
        count: 1,
        facts: new Map([[factKey, { title, note }]]),
      });
    }
  }
  const formatAggregate = (entry: {
    minutes: number;
    count: number;
  }) => `${formatJapaneseDuration(entry.minutes)}（${entry.count}件）`;
  const sortedProjects = [...projectTotals.values()].sort(
    (a, b) => b.minutes - a.minutes || a.label.localeCompare(b.label, "ja"),
  );
  const projects = sortedProjects.map(
    (entry) => `- ${entry.label}: ${formatAggregate(entry)}`,
  );
  const mainWork = sortedProjects
    .map((entry) => {
      const facts = [...entry.facts.values()]
        .map((fact) => {
          if (fact.title === "タイトルなし" && fact.note) return fact.note;
          return `${fact.title}${fact.note ? `（メモ: ${fact.note}）` : ""}`;
        })
        .join("、");
      return `- **${entry.label}**（${formatJapaneseDuration(entry.minutes)}）：${facts}`;
    })
    .join("\n");

  const prompt = `
以下の出力テンプレートに沿って、工数記録から週報を作成してください。
同じプロジェクトや関連する作業はまとめ、工数記録にある事実だけを簡潔に記載してください。
テンプレートの構成を保ち、「主な作業」には確定本文をそのまま使用してください。
確定集計の数値は変更せず、週報の本文だけをMarkdownで返してください。

<出力テンプレート>
${template}
</出力テンプレート>

<主な作業の確定本文>
${mainWork}
</主な作業の確定本文>

<確定集計>
対象期間: ${period}
合計時間: ${formatJapaneseDuration(data.totalMinutes)}
記録件数: ${data.entries.length}件

プロジェクト別:
${projects.join("\n")}
</確定集計>
`.trim();

  return { prompt, mainWork };
}

function replaceMarkdownSection(
  markdown: string,
  heading: string,
  body: string,
): string {
  const lines = markdown.trim().split("\n");
  const headingIndex = lines.findIndex(
    (line) => line.trim() === `## ${heading}`,
  );
  if (headingIndex === -1) return markdown.trim();

  let nextHeadingIndex = lines.findIndex(
    (line, index) => index > headingIndex && /^##\s+/.test(line.trim()),
  );
  if (nextHeadingIndex === -1) nextHeadingIndex = lines.length;

  return [
    ...lines.slice(0, headingIndex + 1),
    "",
    body.trim(),
    "",
    ...lines.slice(nextHeadingIndex),
  ]
    .join("\n")
    .trim();
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
const SUMMARY_COLLAPSED_KEY = "reports.summaryCollapsed";

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
  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: () => apiFetch<UserSettings>("/api/settings"),
  });
  const { data: appConfig } = useQuery({
    queryKey: ["config"],
    queryFn: () => apiFetch<AppConfig>("/api/config"),
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
  const [summaryCollapsed, setSummaryCollapsed] = useState(
    () => localStorage.getItem(SUMMARY_COLLAPSED_KEY) === "1",
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

  useEffect(() => {
    localStorage.setItem(SUMMARY_COLLAPSED_KEY, summaryCollapsed ? "1" : "0");
  }, [summaryCollapsed]);

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
  const reportScrollRef = useRef<HTMLDivElement>(null);
  const [reportScrollbarInset, setReportScrollbarInset] = useState(0);

  useLayoutEffect(() => {
    const element = reportScrollRef.current;
    if (!element) return;

    const updateInset = () => {
      setReportScrollbarInset(element.offsetWidth - element.clientWidth);
    };
    updateInset();

    const observer = new ResizeObserver(updateInset);
    observer.observe(element);
    return () => observer.disconnect();
  }, [rows.length]);

  const queryClient = useQueryClient();
  const [selectedAggregationFormatId, setSelectedAggregationFormatId] = useState("");
  const [weeklyReportOpen, setWeeklyReportOpen] = useState(false);
  const [weeklyReport, setWeeklyReport] = useState("");
  const [weeklyReportError, setWeeklyReportError] = useState("");
  const [weeklyReportGenerating, setWeeklyReportGenerating] = useState(false);
  const [weeklyReportCopied, setWeeklyReportCopied] = useState(false);
  const [aggregationOpen, setAggregationOpen] = useState(false);
  const [aggregationRows, setAggregationRows] = useState<AiAggregationRow[]>([]);
  const [aggregationError, setAggregationError] = useState("");
  const [aggregationGenerating, setAggregationGenerating] = useState(false);
  const [aggregationProgress, setAggregationProgress] =
    useState<AiProgressUpdate | null>(null);
  const [aggregationElapsedSeconds, setAggregationElapsedSeconds] = useState(0);
  const [aggregationCopied, setAggregationCopied] = useState(false);
  const [aggregationTotalMinutes, setAggregationTotalMinutes] = useState(0);
  const [aggregationRestoredAt, setAggregationRestoredAt] = useState<
    number | null
  >(null);

  const aggregationCopyFormats = useMemo(
    () =>
      (settings?.reportCopyFormats ?? []).filter(
        (copyFormat) => copyFormat.target === "ai-aggregation",
      ),
    [settings?.reportCopyFormats],
  );
  const selectedAggregationFormat = aggregationCopyFormats.find(
    (copyFormat) => copyFormat.id === selectedAggregationFormatId,
  );
  const selectedFormatUsesAi = selectedAggregationFormat
    ? reportCopyFormatUsesAi(selectedAggregationFormat)
    : false;
  const aggregationEntriesUrl = buildEntriesUrl(baseFilters, {});
  const { data: aggregationEntries, isFetching: aggregationEntriesFetching } = useQuery({
    queryKey: ["reports-entries", aggregationEntriesUrl],
    queryFn: () => apiFetch<ReportEntriesResponse>(aggregationEntriesUrl),
    enabled: aggregationOpen,
    staleTime: 0,
  });
  const aggregationCacheKey = useMemo(
    () =>
      selectedAggregationFormat && aggregationEntries
        ? buildReportOutputCacheKey({
            range,
            anchor: format(anchor, "yyyy-MM-dd"),
            groupBy,
            clientIds: selectedClientIds,
            projectIds: selectedProjectIds,
            tagIds: selectedTagIds,
            reportRows: rows,
            entries: aggregationEntries.entries,
            totalMinutes: total,
            copyFormat: selectedAggregationFormat,
          })
        : "",
    [
      selectedAggregationFormat,
      aggregationEntries,
      range,
      anchor,
      groupBy,
      selectedClientIds,
      selectedProjectIds,
      selectedTagIds,
      rows,
      total,
    ],
  );

  useEffect(() => {
    if (!aggregationOpen || !aggregationCacheKey || aggregationEntriesFetching) return;
    const cached = readReportOutputCache(localStorage, aggregationCacheKey);
    if (cached) {
      setAggregationRows(cached.rows);
      setAggregationTotalMinutes(cached.totalMinutes);
      setAggregationRestoredAt(cached.createdAt);
      setAggregationError("");
    } else {
      setAggregationRows([]);
      setAggregationRestoredAt(null);
    }
  }, [aggregationOpen, aggregationCacheKey, aggregationEntriesFetching]);

  useEffect(() => {
    if (!aggregationGenerating || !selectedFormatUsesAi) return;
    const startedAt = Date.now();
    setAggregationElapsedSeconds(0);
    const timer = setInterval(() => {
      setAggregationElapsedSeconds(
        Math.floor((Date.now() - startedAt) / 1_000),
      );
    }, 1_000);
    return () => clearInterval(timer);
  }, [aggregationGenerating, selectedFormatUsesAi]);

  useEffect(() => {
    if (aggregationCopyFormats.length === 0) {
      if (selectedAggregationFormatId) setSelectedAggregationFormatId("");
      return;
    }
    if (
      aggregationCopyFormats.length > 0 &&
      !aggregationCopyFormats.some(
        (copyFormat) => copyFormat.id === selectedAggregationFormatId,
      )
    ) {
      setSelectedAggregationFormatId(aggregationCopyFormats[0].id);
    }
  }, [aggregationCopyFormats, selectedAggregationFormatId]);

  async function fetchReportEntries() {
    const url = buildEntriesUrl(baseFilters, {});
    return queryClient.fetchQuery({
      queryKey: ["reports-entries", url],
      queryFn: () => apiFetch<ReportEntriesResponse>(url),
    });
  }

  async function writeClipboard(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
    }
  }

  async function buildCurrentReportRows(): Promise<AiAggregationRow[]> {
    return Promise.all(
      rows.map(async (row) => {
        const extra =
          groupBy === "client"
            ? { clientId: row.key }
            : groupBy === "tag"
              ? { tagId: row.key }
              : { projectId: row.key };
        const url = buildEntriesUrl(baseFilters, extra);
        const entries = await queryClient.fetchQuery({
          queryKey: ["reports-entries", url],
          queryFn: () => apiFetch<ReportEntriesResponse>(url),
        });
        return {
          label:
            groupBy === "project"
              ? row.label.split(" · ").slice(-1)[0]
              : row.label,
          summary: "",
          minutes: row.totalMinutes,
          entryCount: entries.entries.length,
          generatedValues: {},
        };
      }),
    );
  }

  async function createCustomOutput() {
    if (!selectedAggregationFormat) {
      setAggregationError(
        "設定の「レポート」で出力フォーマットを登録してください。",
      );
      return;
    }
    setAggregationGenerating(true);
    setAggregationRows([]);
    setAggregationError("");
    setAggregationCopied(false);
    setAggregationProgress(null);
    setAggregationRestoredAt(null);
    try {
      if (rows.length === 0) {
        setAggregationError("この期間には出力する工数がありません。");
        return;
      }
      setAggregationTotalMinutes(total);
      const entries = await fetchReportEntries();
      const outputCacheKey = buildReportOutputCacheKey({
        range, anchor: format(anchor, "yyyy-MM-dd"), groupBy,
        clientIds: selectedClientIds, projectIds: selectedProjectIds,
        tagIds: selectedTagIds, reportRows: rows, entries: entries.entries,
        totalMinutes: total, copyFormat: selectedAggregationFormat,
      });
      let outputRows: AiAggregationRow[];
      if (reportCopyFormatUsesAi(selectedAggregationFormat)) {
        const prompt = buildReportAggregationPrompt(entries, selectedAggregationFormat);
        const generated = await generateAiTextStream(
          "report-aggregation",
          prompt,
          (progress) => setAggregationProgress(progress),
        );
        outputRows = aggregateAiResult(generated.text, entries);
      } else {
        outputRows = await buildCurrentReportRows();
      }
      setAggregationRows(outputRows);
      writeReportOutputCache(localStorage, {
        key: outputCacheKey,
        createdAt: Date.now(),
        totalMinutes: total,
        rows: outputRows,
      });
    } catch (error) {
      setAggregationError(
        error instanceof Error ? error.message : "出力を作成できませんでした。",
      );
    } finally {
      setAggregationGenerating(false);
    }
  }

  async function copyAggregation() {
    if (!selectedAggregationFormat) return;
    await writeClipboard(
      buildAggregationCopyText(
        aggregationRows,
        aggregationTotalMinutes,
        selectedAggregationFormat,
      ),
    );
    setAggregationCopied(true);
    setTimeout(() => setAggregationCopied(false), 2000);
  }

  async function generateWeeklyReport() {
    setWeeklyReportOpen(true);
    setWeeklyReport("");
    setWeeklyReportError("");
    setWeeklyReportCopied(false);

    if (!settings?.weeklyReportTemplate.trim()) {
      setWeeklyReportError(
        "設定の「レポート」で出力テンプレートを保存してください。",
      );
      return;
    }

    setWeeklyReportGenerating(true);
    try {
      const url = buildEntriesUrl(baseFilters, {});
      const entries = await queryClient.fetchQuery({
        queryKey: ["reports-entries", url],
        queryFn: () => apiFetch<ReportEntriesResponse>(url),
      });
      if (entries.entries.length === 0) {
        setWeeklyReportError("この週には週報に含める工数がありません。");
        return;
      }

      const { prompt, mainWork } = buildWeeklyReportPrompt({
        template: settings.weeklyReportTemplate,
        period: periodLabel(anchor, "week"),
        data: entries,
      });
      const generated = await generateAiText("weekly-report", prompt);
      setWeeklyReport(
        replaceMarkdownSection(generated.text, "主な作業", mainWork),
      );
    } catch (error) {
      setWeeklyReportError(
        typeof error === "string"
          ? error
          : error instanceof Error
            ? error.message
            : "週報を生成できませんでした。",
      );
    } finally {
      setWeeklyReportGenerating(false);
    }
  }

  async function copyWeeklyReport() {
    try {
      await navigator.clipboard.writeText(weeklyReport);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = weeklyReport;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
    }
    setWeeklyReportCopied(true);
    setTimeout(() => setWeeklyReportCopied(false), 2000);
  }

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <ViewToolbar className="sticky top-0 z-40">
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

      <div className="min-h-0 w-full flex-1">
        <div className="flex h-full min-h-0 flex-col bg-white">
          {/* Filters */}
          <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-neutral-200 bg-neutral-50/60 px-3 py-2">
            <button
              type="button"
              onClick={() => setSummaryCollapsed((value) => !value)}
              aria-pressed={!summaryCollapsed}
              aria-label={summaryCollapsed ? "集計サマリーを展開" : "集計サマリーを折りたたむ"}
              title={summaryCollapsed ? "集計サマリーを展開" : "集計サマリーを折りたたむ"}
              className={`flex size-7 items-center justify-center rounded transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-neutral-400 ${
                summaryCollapsed
                  ? "text-neutral-400 hover:bg-neutral-200/70 hover:text-neutral-700"
                  : "text-neutral-700 hover:bg-neutral-200/70"
              }`}
            >
              <ChartPie className="size-4" />
            </button>
            <span className="mr-1 text-xs font-medium text-neutral-500">絞り込み</span>
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
                className="ml-1 text-xs text-neutral-500 underline-offset-2 hover:text-neutral-900 hover:underline"
              >
                すべてクリア
              </button>
            )}
          </div>

          {isLoading ? (
            <div className="flex min-h-0 flex-1 items-center justify-center">
              <div className="h-6 w-6 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-700" />
            </div>
          ) : rows.length === 0 ? (
            <div className="flex min-h-0 flex-1 items-center justify-center text-neutral-400">
              この期間にデータがありません
            </div>
          ) : (
          <div
            className={`grid min-h-0 flex-1 grid-rows-[auto_minmax(0,1fr)] items-stretch xl:grid-rows-1 ${
              summaryCollapsed
                ? "xl:grid-cols-[8px_minmax(0,1fr)]"
                : "xl:grid-cols-[300px_minmax(0,1fr)]"
            }`}
          >
            {/* Summary */}
            <section
              className={`min-w-0 border-b border-neutral-200 xl:border-b-0 xl:border-r ${
                summaryCollapsed ? "h-2 xl:h-auto" : ""
              }`}
            >
              {summaryCollapsed ? (
                <div
                  aria-hidden="true"
                  className="flex h-full w-full flex-row gap-px bg-white xl:flex-col"
                >
                  {rows.map((row, i) => (
                    <span
                      key={row.key}
                      className="min-h-px min-w-px"
                      style={{
                        background: row.color || COLORS[i % COLORS.length],
                        flexBasis: 0,
                        flexGrow: Math.max(row.totalMinutes, 1),
                      }}
                    />
                  ))}
                </div>
              ) : (
                <>
                  <div className="flex min-h-12 shrink-0 items-center justify-between border-b border-transparent px-3 sm:px-4">
                    <h2 className="text-sm font-medium text-neutral-800">集計サマリー</h2>
                    <span className="text-xs text-neutral-400">{rows.length}件</span>
                  </div>
                  <div className="flex flex-col items-center gap-4 px-3 pb-3 pt-2 sm:px-4 sm:pb-4 md:flex-row md:items-stretch xl:flex-col xl:items-center">
                <div className="relative h-56 w-full max-w-56 shrink-0">
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
                <ul className="w-full min-w-0 flex-1 space-y-2.5 self-center text-sm">
                  {rows.map((row, i) => {
                    const color = row.color || COLORS[i % COLORS.length];
                    const displayLabel =
                      groupBy === "project"
                        ? row.label.split(" · ").slice(-1)[0]
                        : row.label;
                    const pct = total > 0 ? (row.totalMinutes / total) * 100 : 0;
                    return (
                      <li key={row.key} className="space-y-1">
                        <div className="flex min-w-0 items-center gap-2">
                          <span
                            className="inline-block h-2.5 w-2.5 shrink-0 rounded-full"
                            style={{ background: color }}
                          />
                          <span className="min-w-0 flex-1 truncate text-neutral-700">
                            {displayLabel}
                          </span>
                          <span className="shrink-0 text-xs tabular-nums text-neutral-700">
                            {formatDuration(row.totalMinutes)}
                          </span>
                          <span className="w-9 shrink-0 text-right text-xs tabular-nums text-neutral-400">
                            {Math.round(pct)}%
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
                </>
              )}
            </section>

            {/* Details */}
            <section className="flex min-h-0 min-w-0 flex-col overflow-hidden">
              <div className="flex min-h-12 shrink-0 flex-wrap items-center justify-between gap-3 border-b border-neutral-200 px-3 py-2.5">
                <h2 className="text-sm font-medium text-neutral-800">明細</h2>
                <div className="flex flex-wrap items-center justify-end gap-3">
                  {range === "week" && (
                    <button
                      type="button"
                      onClick={generateWeeklyReport}
                      disabled={weeklyReportGenerating}
                      className="inline-flex items-center gap-1.5 rounded-md border border-emerald-200 bg-emerald-50 px-2.5 py-1 text-xs font-medium text-emerald-800 hover:bg-emerald-100 disabled:opacity-60"
                    >
                      {weeklyReportGenerating ? (
                        <LoaderCircle className="size-3.5 animate-spin" />
                      ) : (
                        <Sparkles className="size-3.5" />
                      )}
                      {weeklyReportGenerating ? "生成中…" : "週報を生成"}
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => {
                      setAggregationOpen(true);
                      setAggregationError("");
                      setAggregationRows([]);
                      setAggregationProgress(null);
                      setAggregationRestoredAt(null);
                    }}
                    className="inline-flex items-center gap-1.5 rounded-md border border-blue-200 bg-blue-50 px-2.5 py-1 text-xs font-medium text-blue-800 hover:bg-blue-100"
                  >
                    <FileOutput className="size-3.5" />
                    出力を作成
                  </button>
                </div>
              </div>
              <div
                className="shrink-0 overflow-hidden border-b border-neutral-200 bg-neutral-50/80"
                style={{ paddingRight: reportScrollbarInset }}
              >
                <table className="w-full min-w-[760px] table-fixed text-sm">
                  <ReportTableColumns />
                  <thead>
                    <tr className="text-left text-neutral-500">
                      <th className="px-3 py-2 font-medium">
                        <div className="flex items-center gap-1.5">
                          <button
                            type="button"
                            onClick={toggleAll}
                            aria-label={
                              anyExpanded ? "すべて折りたたむ" : "すべて展開"
                            }
                            title={
                              anyExpanded ? "すべて折りたたむ" : "すべて展開"
                            }
                            className="inline-flex size-4 shrink-0 items-center justify-center rounded text-neutral-400 hover:bg-neutral-200/70 hover:text-neutral-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-neutral-400"
                          >
                            {anyExpanded ? (
                              <ChevronsDownUp className="size-3" />
                            ) : (
                              <ChevronsUpDown className="size-3" />
                            )}
                          </button>
                          <span>内容</span>
                          <button
                            type="button"
                            role="switch"
                            aria-checked={groupSameTitles}
                            onClick={() => setGroupSameTitles((value) => !value)}
                            className="ml-2 inline-flex items-center gap-2 text-xs font-normal text-neutral-600"
                          >
                            <span>まとめる</span>
                            <span
                              className={`relative inline-flex h-4 w-7 shrink-0 items-center rounded-full transition-colors ${
                                groupSameTitles ? "bg-neutral-900" : "bg-neutral-300"
                              }`}
                            >
                              <span
                                className={`inline-block h-3 w-3 transform rounded-full bg-white shadow transition-transform ${
                                  groupSameTitles
                                    ? "translate-x-3.5"
                                    : "translate-x-0.5"
                                }`}
                              />
                            </span>
                          </button>
                        </div>
                      </th>
                      <th className="whitespace-nowrap px-3 py-2 font-medium">日時</th>
                      <th className="whitespace-nowrap px-3 py-2 font-medium">タグ</th>
                      <th className="whitespace-nowrap px-3 py-2 text-right font-medium">時間</th>
                      <th className="whitespace-nowrap px-3 py-2 text-right font-medium">割合</th>
                    </tr>
                  </thead>
                </table>
              </div>
              <div
                ref={reportScrollRef}
                className="report-scroll-area min-h-0 flex-1 overflow-auto"
              >
                <table className="w-full min-w-[760px] table-fixed text-sm">
                  <ReportTableColumns />
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
                  </tbody>
                </table>
              </div>
              <div
                className="shrink-0 overflow-hidden border-t border-neutral-200 bg-neutral-50/50"
                style={{ paddingRight: reportScrollbarInset }}
              >
                <table className="w-full min-w-[760px] table-fixed text-sm">
                  <ReportTableColumns />
                  <tbody>
                    <tr className="font-medium">
                      <td colSpan={3} className="px-3 py-2">合計</td>
                      <td className="whitespace-nowrap px-3 py-2 text-right tabular-nums">{formatDuration(total)}</td>
                      <td className="whitespace-nowrap px-3 py-2 text-right tabular-nums">100%</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </section>
          </div>
          )}
        </div>
      </div>

      <Dialog
        open={aggregationOpen}
        onOpenChange={setAggregationOpen}
        contentClassName="flex h-[min(780px,calc(100svh-40px))] w-[min(1200px,calc(100vw-40px))] max-w-none flex-col overflow-hidden"
      >
        <DialogHeader>
          <DialogTitle>カスタム出力</DialogTitle>
          <p className="text-sm text-neutral-500">
            {periodLabel(anchor, range)} · 表示中の絞り込みを反映
          </p>
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden">
          <div className="shrink-0 space-y-1.5">
            <label
              htmlFor="aggregation-copy-format"
              className="text-sm font-medium text-neutral-800"
            >
              出力形式
            </label>
            {aggregationCopyFormats.length > 0 ? (
              <Select
                id="aggregation-copy-format"
                value={selectedAggregationFormatId}
                disabled={aggregationGenerating}
                onChange={(event) => {
                  setSelectedAggregationFormatId(event.target.value);
                  setAggregationRows([]);
                  setAggregationError("");
                  setAggregationProgress(null);
                  setAggregationRestoredAt(null);
                }}
              >
                {aggregationCopyFormats.map((copyFormat) => (
                  <option key={copyFormat.id} value={copyFormat.id}>
                    {copyFormat.name}（{copyFormat.delimiter === "comma" ? "CSV" : "TSV"}・{copyFormat.columns.length}列）
                  </option>
                ))}
              </Select>
            ) : (
              <div className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800">
                設定の「レポート」で出力フォーマットを登録してください。
              </div>
            )}
            {selectedAggregationFormat && (
              <p className="flex items-center gap-2 text-xs text-neutral-500">
                <span
                  className={`rounded-full px-2 py-0.5 font-medium ${
                    selectedFormatUsesAi
                      ? "bg-violet-100 text-violet-700"
                      : "bg-neutral-100 text-neutral-600"
                  }`}
                >
                  {selectedFormatUsesAi ? "AI使用" : "AIなし"}
                </span>
                {selectedFormatUsesAi
                  ? "設定した指示で工数を再分類して出力します。"
                  : "現在の集計結果を指定した列順で出力します。"}
              </p>
            )}
          </div>

          {aggregationGenerating ? (
            <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 text-sm text-neutral-500">
              <div className="flex items-center gap-2">
                <LoaderCircle className="size-5 animate-spin text-blue-700" />
                <span>
                  {selectedFormatUsesAi
                    ? `${aiProviderLabel(appConfig?.aiProvider)}で作成しています…`
                    : "現在の集計から作成しています…"}
                </span>
                {selectedFormatUsesAi && (
                  <span className="font-mono text-xs tabular-nums text-neutral-400">
                    {formatElapsedTime(aggregationElapsedSeconds)}
                  </span>
                )}
              </div>
              {selectedFormatUsesAi && aggregationProgress && (
                <div
                  role="status"
                  aria-live="polite"
                  className="max-h-40 w-full max-w-2xl overflow-auto rounded-lg border border-blue-100 bg-blue-50/60 px-4 py-3 text-left"
                >
                  <div className="mb-1 text-[11px] font-medium text-blue-700">
                    {aggregationProgress.kind === "reasoning"
                      ? "考えていること"
                      : "進捗"}
                  </div>
                  <p className="whitespace-pre-wrap text-xs leading-5 text-neutral-600">
                    {aggregationProgress.message}
                  </p>
                </div>
              )}
            </div>
          ) : aggregationError ? (
            <div
              role="alert"
              className="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm leading-6 text-red-700"
            >
              {aggregationError}
            </div>
          ) : aggregationRows.length > 0 ? (
            <div className="flex min-h-0 flex-1 flex-col gap-2">
            <div className="min-h-0 flex-1 overflow-auto rounded-lg border border-neutral-200">
              <table className="w-full min-w-[680px] text-sm">
                <thead className="sticky top-0 bg-neutral-50 text-left text-neutral-500">
                  <tr>
                    {selectedAggregationFormat?.columns.map((column) => (
                      <th key={column.id} className="whitespace-nowrap px-3 py-2 font-medium">
                        {reportCopyColumnHeader(column)}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody className="divide-y divide-neutral-100">
                  {aggregationRows.map((row, index) => (
                    <tr key={`${row.label}-${index}`}>
                      {selectedAggregationFormat?.columns.map((column) => (
                        <td key={column.id} className="px-3 py-2 text-neutral-700">
                          {aggregationColumnValue(
                            row,
                            column,
                            aggregationTotalMinutes,
                          )}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
              <div className="flex flex-wrap justify-end gap-x-5 gap-y-1 text-xs text-neutral-500">
                {aggregationRestoredAt && (
                  <span className="text-blue-600">
                    保存済み結果を復元 · {new Date(aggregationRestoredAt).toLocaleString("ja-JP")}
                  </span>
                )}
                <span>合計 {formatDuration(aggregationRows.reduce((sum, row) => sum + row.minutes, 0))}</span>
                <span>{aggregationRows.reduce((sum, row) => sum + row.entryCount, 0)}件</span>
              </div>
            </div>
          ) : (
            <div className="flex min-h-0 flex-1 items-center justify-center rounded-lg border border-dashed border-neutral-200 px-4 py-10 text-center text-sm text-neutral-400">
              出力フォーマットを選んで「作成する」を押してください
            </div>
          )}
        </div>

        <DialogFooter className="shrink-0 flex-wrap items-center">
          <Button variant="ghost" onClick={() => setAggregationOpen(false)}>
            閉じる
          </Button>
          <Button
            variant="outline"
            onClick={createCustomOutput}
            disabled={aggregationGenerating || !selectedAggregationFormat}
          >
            {aggregationGenerating ? (
              <LoaderCircle className="size-4 animate-spin" />
            ) : (
              <FileOutput className="size-4" />
            )}
            {aggregationRows.length > 0 ? "再作成" : "作成する"}
          </Button>
          {aggregationRows.length > 0 && !aggregationGenerating && (
            <Button onClick={copyAggregation}>
              {aggregationCopied ? <Check className="size-4" /> : <Copy className="size-4" />}
              {aggregationCopied ? "コピーしました" : "結果をコピー"}
            </Button>
          )}
        </DialogFooter>
      </Dialog>

      <Dialog
        open={weeklyReportOpen}
        onOpenChange={setWeeklyReportOpen}
        contentClassName="w-[min(760px,92vw)]"
      >
        <DialogHeader>
          <DialogTitle>週報</DialogTitle>
          <p className="text-sm text-neutral-500">
            {periodLabel(anchor, "week")} · 表示中の絞り込みを反映
          </p>
        </DialogHeader>

        {weeklyReportGenerating ? (
          <div className="flex min-h-72 flex-col items-center justify-center gap-3 text-sm text-neutral-500">
            <LoaderCircle className="size-6 animate-spin text-emerald-700" />
            {aiProviderLabel(appConfig?.aiProvider)}で生成しています…
          </div>
        ) : weeklyReportError ? (
          <div
            role="alert"
            className="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm leading-6 text-red-700"
          >
            {weeklyReportError}
          </div>
        ) : (
          <MarkdownEditor
            value={weeklyReport}
            onChange={setWeeklyReport}
            showToolbar={false}
            ariaLabel="生成した週報"
            className="weekly-report-markdown-editor h-[min(56vh,520px)] rounded-md border border-neutral-300 bg-white"
          />
        )}

        <DialogFooter className="flex-wrap items-center">
          <Button
            variant="ghost"
            onClick={() => setWeeklyReportOpen(false)}
          >
            閉じる
          </Button>
          {!weeklyReportGenerating && (
            <Button variant="outline" onClick={generateWeeklyReport}>
              <RefreshCw className="size-4" />
              再生成
            </Button>
          )}
          {!!weeklyReport && !weeklyReportGenerating && (
            <Button onClick={copyWeeklyReport}>
              {weeklyReportCopied ? (
                <Check className="size-4" />
              ) : (
                <Copy className="size-4" />
              )}
              {weeklyReportCopied ? "コピーしました" : "週報をコピー"}
            </Button>
          )}
        </DialogFooter>
      </Dialog>
    </div>
  );
}
