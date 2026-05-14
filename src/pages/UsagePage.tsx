import { useQuery } from "@tanstack/react-query";
import { RefreshCw, AlertTriangle } from "lucide-react";
import { apiFetch } from "@/lib/fetcher";
import { cn } from "@/lib/utils";

type Usage = {
  setup_required: boolean;
  asOf?: string;
  resetAt?: string;
  ai?: { ok: boolean; neurons: number; inferences: number; limit: number };
  workers?: { ok: boolean; requests: number; errors: number; limit: number };
  d1?: {
    ok: boolean;
    reads: number;
    writes: number;
    readsLimit: number;
    writesLimit: number;
  };
};

function formatNumber(n: number): string {
  return n.toLocaleString("ja-JP");
}

function formatDateTime(iso?: string): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return d.toLocaleString("ja-JP", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function Meter({
  label,
  used,
  limit,
  unit,
  ok,
}: {
  label: string;
  used: number;
  limit: number;
  unit: string;
  ok: boolean;
}) {
  const pct = Math.min(100, (used / limit) * 100);
  const remaining = Math.max(0, limit - used);
  const barColor =
    pct >= 90
      ? "bg-rose-500"
      : pct >= 60
        ? "bg-amber-500"
        : "bg-emerald-500";

  return (
    <div className="rounded-lg border border-neutral-200 bg-white p-4">
      <div className="flex items-baseline justify-between gap-2">
        <div className="text-sm font-medium text-neutral-700">{label}</div>
        {!ok && (
          <div
            className="flex items-center gap-1 text-xs text-amber-600"
            title="この指標は取得できませんでした"
          >
            <AlertTriangle className="h-3 w-3" />
            取得失敗
          </div>
        )}
      </div>
      <div className="mt-1 flex items-baseline gap-2">
        <span className="text-xl font-semibold tabular-nums">
          {formatNumber(used)}
        </span>
        <span className="text-sm text-neutral-500">
          / {formatNumber(limit)} {unit}
        </span>
        <span className="ml-auto text-xs text-neutral-500 tabular-nums">
          {pct.toFixed(1)}%
        </span>
      </div>
      <div className="mt-2 h-2 overflow-hidden rounded-full bg-neutral-100">
        <div
          className={cn("h-full transition-all duration-500", barColor)}
          style={{ width: `${pct}%` }}
        />
      </div>
      <div className="mt-1 text-xs text-neutral-500 tabular-nums">
        残 {formatNumber(remaining)} {unit}
      </div>
    </div>
  );
}

export function UsagePage() {
  const { data, isFetching, refetch, isError } = useQuery({
    queryKey: ["cloudflare-usage"],
    queryFn: () => apiFetch<Usage>("/api/cloudflare/usage"),
    staleTime: 60_000,
    refetchOnWindowFocus: false,
  });

  return (
    <div className="mx-auto max-w-3xl px-6 py-8">
      <div className="mb-6 flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">使用量</h1>
          <p className="mt-1 text-sm text-neutral-500">
            Cloudflare 無料枠の本日分（00:00 UTC リセット）
          </p>
        </div>
        <button
          onClick={() => refetch()}
          disabled={isFetching}
          className="flex items-center gap-1.5 rounded-md border border-neutral-200 px-3 py-1.5 text-sm hover:bg-neutral-50 disabled:opacity-50"
        >
          <RefreshCw
            className={cn("h-3.5 w-3.5", isFetching && "animate-spin")}
          />
          更新
        </button>
      </div>

      {isError && (
        <div className="rounded-lg border border-rose-200 bg-rose-50 p-4 text-sm text-rose-700">
          取得に失敗しました
        </div>
      )}

      {data?.setup_required && (
        <div className="rounded-lg border border-amber-200 bg-amber-50 p-4 text-sm text-amber-900">
          <div className="font-medium">セットアップが必要です</div>
          <ol className="mt-2 list-decimal space-y-1 pl-5 text-amber-800">
            <li>
              Cloudflare ダッシュボード → My Profile → API Tokens で{" "}
              <code className="rounded bg-amber-100 px-1">Account
              Analytics:Read</code> 権限のトークンを作成
            </li>
            <li>
              <code className="rounded bg-amber-100 px-1">
                wrangler secret put CF_API_TOKEN
              </code>{" "}
              でシークレット登録
            </li>
            <li>
              <code className="rounded bg-amber-100 px-1">wrangler.jsonc</code>{" "}
              の <code className="rounded bg-amber-100 px-1">vars.CF_ACCOUNT_ID</code> に
              アカウント ID を設定して再デプロイ
            </li>
          </ol>
        </div>
      )}

      {data && !data.setup_required && (
        <>
          <div className="space-y-3">
            {data.ai && (
              <Meter
                label="Workers AI"
                used={data.ai.neurons}
                limit={data.ai.limit}
                unit="Neurons"
                ok={data.ai.ok}
              />
            )}
            {data.workers && (
              <Meter
                label="Workers リクエスト"
                used={data.workers.requests}
                limit={data.workers.limit}
                unit="req"
                ok={data.workers.ok}
              />
            )}
            {data.d1 && (
              <>
                <Meter
                  label="D1 読み込み"
                  used={data.d1.reads}
                  limit={data.d1.readsLimit}
                  unit="queries"
                  ok={data.d1.ok}
                />
                <Meter
                  label="D1 書き込み"
                  used={data.d1.writes}
                  limit={data.d1.writesLimit}
                  unit="queries"
                  ok={data.d1.ok}
                />
              </>
            )}
          </div>

          <div className="mt-6 text-xs text-neutral-500">
            最終更新: {formatDateTime(data.asOf)} ／ 次回リセット:{" "}
            {formatDateTime(data.resetAt)} (UTC 0時)
          </div>
        </>
      )}
    </div>
  );
}
