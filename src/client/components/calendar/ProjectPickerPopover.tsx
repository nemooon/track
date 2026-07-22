"use client";

import * as React from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "@client/lib/fetcher";
import type { Project, Tag } from "@shared/types";

type Picked = {
  id: string | null;
  name: string;
  color: string;
  lockedTagIds: string[];
};

const WIDTH = 280;

export function ProjectPickerPopover({
  anchor,
  projects,
  tags,
  initialTitle,
  onPick,
  onCancel,
}: {
  anchor: { left: number; top: number };
  projects: Project[];
  tags: Tag[];
  initialTitle?: string;
  onPick: (
    projectId: string | null,
    title: string,
    tagIds: string[],
    note: string,
  ) => void;
  onCancel: () => void;
}) {
  const [mounted, setMounted] = React.useState(false);
  const [step, setStep] = React.useState<"project" | "detail">("project");
  const [picked, setPicked] = React.useState<Picked | null>(null);
  const [query, setQuery] = React.useState("");
  const [title, setTitle] = React.useState(initialTitle ?? "");
  const [selectedTagIds, setSelectedTagIds] = React.useState<string[]>([]);
  const [note, setNote] = React.useState("");
  const [showSuggestions, setShowSuggestions] = React.useState(false);
  const [activeIndex, setActiveIndex] = React.useState(-1);
  const ref = React.useRef<HTMLDivElement>(null);
  const suggestionListRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => setMounted(true), []);

  React.useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!ref.current) return;
      if (!ref.current.contains(e.target as Node)) onCancel();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [onCancel]);

  const pickedKey = picked ? picked.id ?? "none" : null;
  const titlesQ = useQuery<string[]>({
    queryKey: ["entries", "titles", pickedKey],
    queryFn: () => {
      const param = picked ? picked.id ?? "none" : "";
      const qs = param ? `?projectId=${encodeURIComponent(param)}` : "";
      return apiFetch<string[]>(`/api/entries/titles${qs}`);
    },
    enabled: step === "detail",
    staleTime: 60_000,
  });
  const allTitles = titlesQ.data ?? [];

  const matches = React.useMemo(() => {
    const q = title.trim().toLowerCase();
    const filtered = q
      ? allTitles.filter((t) => t.toLowerCase().startsWith(q) && t.toLowerCase() !== q)
      : allTitles;
    return filtered.slice(0, 8);
  }, [allTitles, title]);

  const visibleSuggestions = showSuggestions && matches.length > 0;

  React.useEffect(() => {
    if (activeIndex >= matches.length) setActiveIndex(matches.length - 1);
  }, [matches.length, activeIndex]);

  React.useEffect(() => {
    if (!visibleSuggestions || activeIndex < 0) return;
    const list = suggestionListRef.current;
    const el = list?.children[activeIndex] as HTMLElement | undefined;
    if (el && list) {
      const top = el.offsetTop;
      const bottom = top + el.offsetHeight;
      if (top < list.scrollTop) list.scrollTop = top;
      else if (bottom > list.scrollTop + list.clientHeight) {
        list.scrollTop = bottom - list.clientHeight;
      }
    }
  }, [activeIndex, visibleSuggestions]);

  function commitTitle(t: string) {
    setTitle(t);
    setShowSuggestions(false);
    setActiveIndex(-1);
  }

  function submit() {
    if (!picked) return;
    const merged = Array.from(
      new Set([...picked.lockedTagIds, ...selectedTagIds]),
    );
    onPick(picked.id, title, merged, note);
  }

  function onTitleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.nativeEvent.isComposing) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (matches.length === 0) return;
      setShowSuggestions(true);
      setActiveIndex((i) => (i < matches.length - 1 ? i + 1 : 0));
    } else if (e.key === "ArrowUp") {
      if (!visibleSuggestions) return;
      e.preventDefault();
      setActiveIndex((i) => (i > 0 ? i - 1 : matches.length - 1));
    } else if (e.key === "Enter") {
      if (e.metaKey || e.ctrlKey) {
        e.preventDefault();
        submit();
      } else if (visibleSuggestions && activeIndex >= 0) {
        e.preventDefault();
        commitTitle(matches[activeIndex]);
      }
    } else if (e.key === "Escape") {
      if (visibleSuggestions) {
        e.preventDefault();
        e.stopPropagation();
        setShowSuggestions(false);
        setActiveIndex(-1);
      }
    }
  }

  function toggleTag(tagId: string) {
    if (picked?.lockedTagIds.includes(tagId)) return;
    setSelectedTagIds((prev) =>
      prev.includes(tagId) ? prev.filter((id) => id !== tagId) : [...prev, tagId],
    );
  }

  function goToDetail(p: Picked) {
    setPicked(p);
    setStep("detail");
    setShowSuggestions(true);
    setActiveIndex(-1);
  }

  const filtered = projects.filter((p) => {
    if (!query) return true;
    const q = query.toLowerCase();
    return (
      p.name.toLowerCase().includes(q) || p.client.name.toLowerCase().includes(q)
    );
  });

  const grouped = new Map<string, Project[]>();
  for (const p of filtered) {
    const list = grouped.get(p.client.name) ?? [];
    list.push(p);
    grouped.set(p.client.name, list);
  }

  // Lock position on mount. Re-clamping on every render makes the popover jump
  // when the soft keyboard opens/closes (changing window.innerHeight).
  const [position] = React.useState(() => ({
    left: Math.min(anchor.left, window.innerWidth - WIDTH),
    top: Math.min(anchor.top, window.innerHeight - 320),
  }));

  if (!mounted) return null;

  return createPortal(
    <div
      ref={ref}
      className="fixed z-50 overflow-hidden rounded-md border border-neutral-200 bg-white shadow-lg"
      style={{ left: position.left, top: position.top, width: WIDTH }}
    >
      {step === "project" ? (
        <>
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="プロジェクトを検索"
            className="w-full border-b border-neutral-200 px-3 py-2 text-sm focus:outline-none"
          />
          <div className="max-h-80 overflow-auto py-1">
            {grouped.size === 0 && (
              <div className="px-3 py-4 text-center text-xs text-neutral-400">
                プロジェクトがありません
              </div>
            )}
            {Array.from(grouped.entries()).map(([clientName, ps]) => (
              <div key={clientName}>
                <div className="px-3 pb-1 pt-2 text-[10px] font-medium uppercase tracking-wider text-neutral-400">
                  {clientName}
                </div>
                {ps.map((p) => (
                  <button
                    key={p.id}
                    onClick={() =>
                      goToDetail({
                        id: p.id,
                        name: p.name,
                        color: p.color,
                        lockedTagIds: p.tags?.map((t) => t.tagId) ?? [],
                      })
                    }
                    className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-neutral-100"
                  >
                    <span
                      className="h-2.5 w-2.5 rounded-full"
                      style={{ backgroundColor: p.color }}
                    />
                    {p.name}
                  </button>
                ))}
              </div>
            ))}
            <button
              onClick={() =>
                goToDetail({
                  id: null,
                  name: "プロジェクトなし",
                  color: "#a3a3a3",
                  lockedTagIds: [],
                })
              }
              className="mt-1 flex w-full items-center gap-2 border-t border-neutral-200 px-3 py-1.5 text-left text-sm text-neutral-500 hover:bg-neutral-100"
            >
              <span className="h-2.5 w-2.5 rounded-full bg-neutral-400" />
              プロジェクトなし
            </button>
          </div>
        </>
      ) : picked ? (
        <>
          <div className="flex items-center gap-2 border-b border-neutral-200 bg-neutral-50 px-2 py-1.5">
            <button
              type="button"
              onClick={() => setStep("project")}
              aria-label="プロジェクト選択に戻る"
              className="rounded p-1 text-neutral-500 hover:bg-neutral-200 hover:text-neutral-800"
            >
              <svg
                viewBox="0 0 16 16"
                className="h-3.5 w-3.5"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path d="M10 3 L5 8 L10 13" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
            <span
              className="h-2.5 w-2.5 shrink-0 rounded-full"
              style={{ backgroundColor: picked.color }}
            />
            <span className="truncate text-sm text-neutral-700">{picked.name}</span>
          </div>
          <input
            autoFocus
            value={title}
            onChange={(e) => {
              setTitle(e.target.value);
              setShowSuggestions(true);
              setActiveIndex(-1);
            }}
            onKeyDown={onTitleKeyDown}
            maxLength={100}
            placeholder="タイトル（例: 定例ミーティング）"
            className="w-full border-b border-neutral-200 px-3 py-2 text-sm focus:outline-none"
          />
          {visibleSuggestions && (
            <div
              ref={suggestionListRef}
              className="max-h-40 overflow-auto border-b border-neutral-200 bg-neutral-50 py-1"
            >
              {matches.map((t, i) => (
                <button
                  key={t}
                  type="button"
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => commitTitle(t)}
                  onMouseEnter={() => setActiveIndex(i)}
                  className={`block w-full truncate px-3 py-1 text-left text-sm ${
                    i === activeIndex ? "bg-neutral-200" : "hover:bg-neutral-100"
                  }`}
                >
                  {t}
                </button>
              ))}
            </div>
          )}
          {tags.length > 0 && (
            <div className="border-b border-neutral-200 px-3 py-2">
              <div className="mb-1 text-[10px] font-medium uppercase tracking-wider text-neutral-400">
                タグ
              </div>
              <div className="flex flex-wrap gap-1">
                {tags.map((tag) => {
                  const locked = picked.lockedTagIds.includes(tag.id);
                  const active = locked || selectedTagIds.includes(tag.id);
                  return (
                    <button
                      key={tag.id}
                      type="button"
                      onClick={() => toggleTag(tag.id)}
                      disabled={locked}
                      title={locked ? "プロジェクトに紐づいているタグ" : undefined}
                      className={`rounded-full border px-2 py-0.5 text-[11px] transition-colors disabled:cursor-default ${
                        active
                          ? "border-transparent text-white"
                          : "border-neutral-200 bg-white text-neutral-600 hover:bg-neutral-50"
                      }`}
                      style={active ? { backgroundColor: tag.color } : undefined}
                    >
                      {tag.name}
                    </button>
                  );
                })}
              </div>
            </div>
          )}
          <div className="border-b border-neutral-200 px-3 py-2">
            <div className="mb-1 text-[10px] font-medium uppercase tracking-wider text-neutral-400">
              メモ
            </div>
            <textarea
              value={note}
              onChange={(e) => setNote(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                  e.preventDefault();
                  submit();
                }
              }}
              maxLength={500}
              rows={2}
              placeholder="任意"
              className="w-full resize-none rounded border border-neutral-200 px-2 py-1 text-xs focus:border-neutral-400 focus:outline-none"
            />
          </div>
          <div className="flex justify-end px-3 py-2">
            <button
              type="button"
              onClick={submit}
              className="rounded bg-neutral-900 px-3 py-1 text-xs text-white hover:bg-neutral-700"
            >
              作成 (⌘↵)
            </button>
          </div>
        </>
      ) : null}
    </div>,
    document.body,
  );
}
