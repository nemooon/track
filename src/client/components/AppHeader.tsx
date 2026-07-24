import { useEffect, useRef, useState } from "react";
import { CalendarDays, BarChart3, Ellipsis, Settings } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Link, useLocation } from "react-router";
import { useAppUi } from "@client/components/AppUiContext";
import { cn } from "@client/lib/utils";

const views = [
  { href: "/calendar", label: "カレンダー", icon: CalendarDays },
  { href: "/reports", label: "レポート", icon: BarChart3 },
];

export function AppHeader() {
  const { pathname } = useLocation();
  const { openSettings } = useAppUi();
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;

    function onPointerDown(event: MouseEvent) {
      if (!menuRef.current?.contains(event.target as Node)) setMenuOpen(false);
    }

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setMenuOpen(false);
    }

    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [menuOpen]);

  function startWindowDrag(event: React.MouseEvent<HTMLElement>) {
    if (
      event.button !== 0 ||
      !("__TAURI_INTERNALS__" in window) ||
      (event.target as HTMLElement).closest(
        "a, button, input, select, textarea, [role='menu'], [role='dialog']",
      )
    ) {
      return;
    }

    event.preventDefault();
    void getCurrentWindow().startDragging();
  }

  return (
    <header
      onMouseDown={startWindowDrag}
      className="relative flex h-11 shrink-0 items-center border-b border-[#1d2824] bg-[#2e3a35] pl-[92px] pr-3"
    >
      <nav className="inline-flex h-full items-center gap-1">
        {views.map(({ href, label, icon: Icon }) => {
          const active = pathname === href;
          return (
            <Link
              key={href}
              to={href}
              title={`${label}（⌘${href === "/calendar" ? "1" : "2"}）`}
              aria-keyshortcuts={`Meta+${href === "/calendar" ? "1" : "2"}`}
              className={cn(
                "relative inline-flex h-full shrink-0 items-center justify-center gap-1.5 px-3 text-xs font-medium transition-colors after:absolute after:inset-x-3 after:bottom-0 after:h-0.5 after:rounded-full after:transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-white/60",
                active
                  ? "text-white after:bg-white"
                  : "text-white/55 after:bg-transparent hover:text-white/85",
              )}
            >
              <Icon className="size-4" />
              {label}
            </Link>
          );
        })}
      </nav>

      <div ref={menuRef} className="relative ml-auto shrink-0">
        <button
          type="button"
          onClick={() => setMenuOpen((current) => !current)}
          aria-label="アプリメニュー"
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          className={cn(
            "inline-flex size-8 items-center justify-center rounded-md text-white/65 transition-colors hover:bg-white/10 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/60",
            menuOpen && "bg-[#f3f5f4] text-[#26332e]",
          )}
        >
          <Ellipsis className="size-5" />
        </button>

        {menuOpen && (
          <div
            role="menu"
            className="absolute right-0 top-full z-50 mt-1 min-w-44 rounded-lg border border-neutral-200 bg-white p-1 shadow-lg"
          >
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setMenuOpen(false);
                openSettings();
              }}
              className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm text-neutral-700 hover:bg-neutral-100"
            >
              <Settings className="size-4 text-neutral-500" />
              <span>設定…</span>
              <kbd className="ml-auto text-xs text-neutral-400">⌘,</kbd>
            </button>
          </div>
        )}
      </div>
    </header>
  );
}
