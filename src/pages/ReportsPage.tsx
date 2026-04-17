import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell } from "recharts";
import { addDays, addMonths } from "date-fns";
import { Button } from "@/components/ui/button";
import { apiFetch } from "@/lib/fetcher";
import { formatWeekLabel, formatMonthLabel, getWeekRange, getMonthRange } from "@/lib/time";
import type { ReportResponse } from "@/types";

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
  const [groupBy, setGroupBy] = useState<"client" | "project">("project");

  const { data, isLoading } = useQuery({
    queryKey: ["reports", range, anchor.toISOString(), groupBy],
    queryFn: () =>
      apiFetch<ReportResponse>(
        `/api/reports?range=${range}&anchor=${anchor.toISOString()}&groupBy=${groupBy}`,
      ),
  });

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
    <div className="mx-auto max-w-4xl space-y-6 p-6">
      {/* Controls */}
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
          {(["client", "project"] as const).map((g) => (
            <button
              key={g}
              onClick={() => setGroupBy(g)}
              className={`rounded px-3 py-1 text-sm ${groupBy === g ? "bg-neutral-900 text-white" : "text-neutral-600 hover:bg-neutral-100"}`}
            >
              {g === "client" ? "クライアント" : "プロジェクト"}
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
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-neutral-200 text-left text-neutral-500">
              <th className="py-2 font-medium">ラベル</th>
              <th className="py-2 text-right font-medium">時間</th>
              <th className="py-2 text-right font-medium">割合</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row, i) => (
              <tr key={row.key} className="border-b border-neutral-100">
                <td className="flex items-center gap-2 py-2">
                  <span
                    className="inline-block h-3 w-3 rounded-sm"
                    style={{ background: row.color || COLORS[i % COLORS.length] }}
                  />
                  {row.label}
                </td>
                <td className="py-2 text-right">{formatDuration(row.totalMinutes)}</td>
                <td className="py-2 text-right">
                  {total > 0 ? Math.round((row.totalMinutes / total) * 100) : 0}%
                </td>
              </tr>
            ))}
            <tr className="font-medium">
              <td className="py-2">合計</td>
              <td className="py-2 text-right">{formatDuration(total)}</td>
              <td className="py-2 text-right">100%</td>
            </tr>
          </tbody>
        </table>
      )}
    </div>
  );
}
