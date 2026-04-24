"use client";

import * as React from "react";
import { createPortal } from "react-dom";
import type { Project, Tag } from "@/types";

export function ProjectPickerPopover({
  anchor,
  projects,
  tags,
  onPick,
  onCancel,
}: {
  anchor: { left: number; top: number };
  projects: Project[];
  tags: Tag[];
  onPick: (projectId: string | null, title: string, tagIds: string[]) => void;
  onCancel: () => void;
}) {
  const [mounted, setMounted] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const [title, setTitle] = React.useState("");
  const [selectedTagIds, setSelectedTagIds] = React.useState<string[]>([]);
  const ref = React.useRef<HTMLDivElement>(null);

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

  function toggleTag(tagId: string) {
    setSelectedTagIds((prev) =>
      prev.includes(tagId) ? prev.filter((id) => id !== tagId) : [...prev, tagId],
    );
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

  if (!mounted) return null;

  // clamp to viewport
  const left = Math.min(anchor.left, window.innerWidth - 260);
  const top = Math.min(anchor.top, window.innerHeight - 320);

  return createPortal(
    <div
      ref={ref}
      className="fixed z-50 w-[240px] overflow-hidden rounded-md border border-neutral-200 bg-white shadow-lg"
      style={{ left, top }}
    >
      <input
        autoFocus
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.nativeEvent.isComposing) {
            onPick(null, title, selectedTagIds);
          }
        }}
        maxLength={100}
        placeholder="タイトル（例: 定例ミーティング）"
        className="w-full border-b border-neutral-200 px-3 py-2 text-sm focus:outline-none"
      />
      <input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="プロジェクトを検索"
        className="w-full border-b border-neutral-200 px-3 py-2 text-sm focus:outline-none"
      />
      <div className="max-h-[280px] overflow-auto py-1">
        <button
          onClick={() => onPick(null, title, selectedTagIds)}
          className="flex w-full items-center gap-2 border-b border-neutral-200 px-3 py-1.5 text-left text-sm hover:bg-neutral-100"
        >
          <span
            className="h-2.5 w-2.5 rounded-full bg-neutral-400"
          />
          プロジェクトなし
        </button>
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
                onClick={() => onPick(p.id, title, selectedTagIds)}
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
      </div>
      {tags.length > 0 && (
        <div className="border-t border-neutral-200 px-3 py-2">
          <div className="mb-1 text-[10px] font-medium uppercase tracking-wider text-neutral-400">
            タグ
          </div>
          <div className="flex flex-wrap gap-1">
            {tags.map((tag) => (
              <button
                key={tag.id}
                type="button"
                onClick={() => toggleTag(tag.id)}
                className={`rounded-full border px-2 py-0.5 text-[11px] transition-colors ${
                  selectedTagIds.includes(tag.id)
                    ? "border-transparent text-white"
                    : "border-neutral-200 bg-white text-neutral-600 hover:bg-neutral-50"
                }`}
                style={
                  selectedTagIds.includes(tag.id) ? { backgroundColor: tag.color } : undefined
                }
              >
                {tag.name}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>,
    document.body,
  );
}
