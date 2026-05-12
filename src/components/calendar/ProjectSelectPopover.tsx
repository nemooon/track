"use client";

import * as React from "react";
import { createPortal } from "react-dom";
import type { Project } from "@/types";

const WIDTH = 280;

export function ProjectSelectPopover({
  anchor,
  projects,
  onPick,
  onCancel,
}: {
  anchor: { left: number; top: number };
  projects: Project[];
  onPick: (projectId: string | null) => void;
  onCancel: () => void;
}) {
  const [mounted, setMounted] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const ref = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => setMounted(true), []);

  React.useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!ref.current) return;
      if (!ref.current.contains(e.target as Node)) onCancel();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onCancel();
      }
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [onCancel]);

  const filtered = projects
    .filter((p) => !p.archived)
    .filter((p) => {
      if (!query) return true;
      const q = query.toLowerCase();
      return (
        p.name.toLowerCase().includes(q) ||
        p.client.name.toLowerCase().includes(q)
      );
    });

  const grouped = new Map<string, Project[]>();
  for (const p of filtered) {
    const list = grouped.get(p.client.name) ?? [];
    list.push(p);
    grouped.set(p.client.name, list);
  }

  if (!mounted) return null;

  const left = Math.min(anchor.left, window.innerWidth - WIDTH - 8);
  const top = Math.min(anchor.top, window.innerHeight - 320);

  return createPortal(
    <div
      ref={ref}
      className="fixed z-[60] overflow-hidden rounded-md border border-neutral-200 bg-white shadow-lg"
      style={{ left, top, width: WIDTH }}
    >
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
                type="button"
                onClick={() => onPick(p.id)}
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
          type="button"
          onClick={() => onPick(null)}
          className="mt-1 flex w-full items-center gap-2 border-t border-neutral-200 px-3 py-1.5 text-left text-sm text-neutral-500 hover:bg-neutral-100"
        >
          <span className="h-2.5 w-2.5 rounded-full bg-neutral-400" />
          プロジェクトなし
        </button>
      </div>
    </div>,
    document.body,
  );
}
