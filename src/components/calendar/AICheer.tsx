import { useQuery } from "@tanstack/react-query";
import { Sparkles, RefreshCw } from "lucide-react";
import { apiFetch } from "@/lib/fetcher";

export function AICheer() {
  const { data, isFetching, refetch } = useQuery({
    queryKey: ["ai-cheer"],
    queryFn: () => apiFetch<{ message: string }>("/api/ai/cheer"),
    staleTime: 60 * 60 * 1000,
    refetchOnWindowFocus: false,
  });

  return (
    <div className="flex items-center gap-2 border-b border-neutral-200 bg-gradient-to-r from-amber-50 via-rose-50 to-violet-50 px-6 py-2 text-sm text-neutral-700">
      <Sparkles className="h-4 w-4 shrink-0 text-amber-500" />
      <span className="min-w-0 flex-1 truncate">
        {isFetching && !data
          ? "AI が記録を眺めています…"
          : (data?.message ?? "—")}
      </span>
      <button
        type="button"
        onClick={() => refetch()}
        disabled={isFetching}
        title="もう一言もらう"
        className="rounded p-1 text-neutral-500 hover:bg-white/60 hover:text-neutral-700 disabled:opacity-50"
      >
        <RefreshCw className={`h-3.5 w-3.5 ${isFetching ? "animate-spin" : ""}`} />
      </button>
    </div>
  );
}
