import * as React from "react";
import { createPortal } from "react-dom";

export type FilterOption = {
  id: string;
  label: string;
  hint?: string;
  color?: string;
};

export function FilterMultiSelect({
  label,
  options,
  selected,
  onChange,
  placeholder,
}: {
  label: string;
  options: FilterOption[];
  selected: string[];
  onChange: (next: string[]) => void;
  placeholder?: string;
}) {
  const [open, setOpen] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const [anchor, setAnchor] = React.useState<{ left: number; top: number; width: number } | null>(
    null,
  );
  const triggerRef = React.useRef<HTMLButtonElement>(null);
  const popRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (popRef.current?.contains(t)) return;
      if (triggerRef.current?.contains(t)) return;
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  function toggle() {
    if (open) {
      setOpen(false);
      return;
    }
    const rect = triggerRef.current?.getBoundingClientRect();
    if (rect) {
      setAnchor({ left: rect.left, top: rect.bottom + 4, width: Math.max(rect.width, 240) });
    }
    setQuery("");
    setOpen(true);
  }

  function toggleId(id: string) {
    onChange(selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id]);
  }

  const filtered = query
    ? options.filter((o) => {
        const q = query.toLowerCase();
        return o.label.toLowerCase().includes(q) || o.hint?.toLowerCase().includes(q);
      })
    : options;

  const selectedCount = selected.length;
  const displayLabel =
    selectedCount === 0
      ? placeholder ?? `すべての${label}`
      : selectedCount === 1
        ? options.find((o) => o.id === selected[0])?.label ?? `${label}: 1`
        : `${label}: ${selectedCount}件`;

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={toggle}
        className={`flex h-8 items-center gap-1.5 rounded-md border px-3 text-xs transition-colors ${
          selectedCount > 0
            ? "border-neutral-900 bg-neutral-900 text-white"
            : "border-neutral-300 bg-white text-neutral-700 hover:bg-neutral-50"
        }`}
      >
        <span>{displayLabel}</span>
        <svg
          width="10"
          height="10"
          viewBox="0 0 10 10"
          fill="none"
          className="opacity-70"
        >
          <path d="M2 4l3 3 3-3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {open &&
        anchor &&
        createPortal(
          <div
            ref={popRef}
            className="fixed z-50 overflow-hidden rounded-md border border-neutral-200 bg-white shadow-lg"
            style={{
              left: Math.min(anchor.left, window.innerWidth - anchor.width - 8),
              top: Math.min(anchor.top, window.innerHeight - 360),
              width: anchor.width,
            }}
          >
            <div className="flex items-center justify-between border-b border-neutral-200 px-3 py-2">
              <input
                autoFocus
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={`${label}を検索`}
                className="flex-1 text-sm focus:outline-none"
              />
              {selectedCount > 0 && (
                <button
                  type="button"
                  onClick={() => onChange([])}
                  className="ml-2 text-[11px] text-neutral-500 hover:text-neutral-900"
                >
                  クリア
                </button>
              )}
            </div>
            <div className="max-h-[300px] overflow-auto py-1">
              {filtered.length === 0 && (
                <div className="px-3 py-4 text-center text-xs text-neutral-400">
                  該当する{label}がありません
                </div>
              )}
              {filtered.map((opt) => {
                const checked = selected.includes(opt.id);
                return (
                  <button
                    key={opt.id}
                    type="button"
                    onClick={() => toggleId(opt.id)}
                    className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-neutral-100"
                  >
                    <span
                      className={`flex h-4 w-4 shrink-0 items-center justify-center rounded border ${
                        checked ? "border-neutral-900 bg-neutral-900 text-white" : "border-neutral-300 bg-white"
                      }`}
                    >
                      {checked && (
                        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                          <path d="M2 5l2 2 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                      )}
                    </span>
                    {opt.color && (
                      <span
                        className="h-2.5 w-2.5 shrink-0 rounded-full"
                        style={{ backgroundColor: opt.color }}
                      />
                    )}
                    <span className="flex-1 truncate">{opt.label}</span>
                    {opt.hint && (
                      <span className="text-[10px] text-neutral-400">{opt.hint}</span>
                    )}
                  </button>
                );
              })}
            </div>
          </div>,
          document.body,
        )}
    </>
  );
}
