import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Sparkles, RefreshCw } from "lucide-react";
import { apiFetch } from "@/lib/fetcher";

const SCROLL_PX_PER_SEC = 60;
const PAUSE_MS = 2000;

export function AICheer() {
  const { data, isFetching, refetch } = useQuery({
    queryKey: ["ai-cheer"],
    queryFn: () => apiFetch<{ message: string }>("/api/ai/cheer"),
    staleTime: 60 * 60 * 1000,
    refetchOnWindowFocus: false,
  });

  const text = isFetching && !data ? "AI が記録を眺めています…" : (data?.message ?? "—");

  const containerRef = useRef<HTMLDivElement>(null);
  const measureRef = useRef<HTMLSpanElement>(null);
  const trackRef = useRef<HTMLDivElement>(null);
  const [overflowing, setOverflowing] = useState(false);

  useEffect(() => {
    const container = containerRef.current;
    const measureEl = measureRef.current;
    if (!container || !measureEl) return;
    const check = () => {
      setOverflowing(measureEl.scrollWidth > container.clientWidth + 1);
    };
    check();
    const ro = new ResizeObserver(check);
    ro.observe(container);
    return () => ro.disconnect();
  }, [text]);

  useEffect(() => {
    if (!overflowing) return;
    const track = trackRef.current;
    const first = track?.firstElementChild as HTMLElement | null;
    if (!track || !first || first.offsetWidth === 0) return;
    const copyWidth = first.offsetWidth;
    const scrollMs = (copyWidth / SCROLL_PX_PER_SEC) * 1000;
    const totalMs = scrollMs + PAUSE_MS;
    const pauseOffset = PAUSE_MS / totalMs;
    const animation = track.animate(
      [
        { transform: "translateX(0)", offset: 0 },
        { transform: "translateX(0)", offset: pauseOffset },
        { transform: "translateX(-50%)", offset: 1 },
      ],
      { duration: totalMs, iterations: Infinity, easing: "linear" },
    );
    return () => animation.cancel();
  }, [overflowing, text]);

  return (
    <div className="flex items-center gap-2 border-b border-neutral-200 bg-gradient-to-r from-amber-50 via-rose-50 to-violet-50 px-3 py-2 text-sm text-neutral-700 sm:px-6">
      <Sparkles className="h-4 w-4 shrink-0 text-amber-500" />
      <div ref={containerRef} className="relative min-w-0 flex-1 overflow-hidden">
        <span
          ref={measureRef}
          aria-hidden
          className="invisible pointer-events-none absolute whitespace-nowrap"
        >
          {text}
        </span>
        {overflowing ? (
          <div ref={trackRef} className="inline-flex whitespace-nowrap">
            <span className="pr-12">{text}</span>
            <span aria-hidden className="pr-12">
              {text}
            </span>
          </div>
        ) : (
          <span className="block truncate">{text}</span>
        )}
      </div>
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
