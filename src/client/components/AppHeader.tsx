import { useEffect, useRef, useState } from "react";
import {
  BarChart3,
  Bot,
  CalendarDays,
  Ellipsis,
  ExternalLink,
  Info,
  Keyboard,
  Settings,
} from "lucide-react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Link, useLocation } from "react-router";
import { useAppUi } from "@client/components/AppUiContext";
import {
  Dialog,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@client/components/ui/dialog";
import { cn } from "@client/lib/utils";
import packageJson from "../../../package.json";
import appIconUrl from "../../../src-tauri/icons/128x128.png";

const REPOSITORY_URL = "https://github.com/nemooon/track";

const views = [
  { href: "/calendar", label: "カレンダー", icon: CalendarDays },
  { href: "/reports", label: "レポート", icon: BarChart3 },
];

const shortcutGroups = [
  {
    label: "画面",
    items: [
      ["カレンダーを開く", "⌘1"],
      ["レポートを開く", "⌘2"],
      ["設定を開く", "⌘,"],
    ],
  },
  {
    label: "日付",
    items: [
      ["前の期間へ移動", "⌘["],
      ["次の期間へ移動", "⌘]"],
      ["今日へ移動", "⌘T"],
    ],
  },
  {
    label: "カレンダー",
    items: [
      ["表示を拡大", "⌘+"],
      ["表示を縮小", "⌘−"],
    ],
  },
] as const;

export function AppHeader() {
  const { pathname } = useLocation();
  const { openSettings } = useAppUi();
  const [menuOpen, setMenuOpen] = useState(false);
  const [openDialog, setOpenDialog] = useState<"shortcuts" | "about" | null>(
    null,
  );
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

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;

    let active = true;
    let unlisten: UnlistenFn | undefined;
    void listen("track-open-about", () => setOpenDialog("about")).then(
      (dispose) => {
        if (active) unlisten = dispose;
        else dispose();
      },
    );

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

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
            className="absolute right-0 top-full z-50 mt-1 min-w-56 rounded-lg border border-neutral-200 bg-white p-1 shadow-lg"
          >
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setMenuOpen(false);
                setOpenDialog("shortcuts");
              }}
              className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm text-neutral-700 hover:bg-neutral-100"
            >
              <Keyboard className="size-4 text-neutral-500" />
              <span>キーボードショートカット</span>
            </button>
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
            {"__TAURI_INTERNALS__" in window && (
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setMenuOpen(false);
                  void invoke("show_ai_integration_installer");
                }}
                className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm text-neutral-700 hover:bg-neutral-100"
              >
                <Bot className="size-4 text-neutral-500" />
                <span>AI連携をインストール…</span>
              </button>
            )}
            <div
              role="separator"
              className="mx-2 my-1 h-px bg-neutral-200"
            />
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setMenuOpen(false);
                setOpenDialog("about");
              }}
              className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm text-neutral-700 hover:bg-neutral-100"
            >
              <Info className="size-4 text-neutral-500" />
              <span>Trackについて</span>
            </button>
          </div>
        )}
      </div>

      <Dialog
        open={openDialog === "shortcuts"}
        onOpenChange={(open) => {
          if (!open) setOpenDialog(null);
        }}
      >
        <DialogHeader>
          <DialogTitle>キーボードショートカット</DialogTitle>
        </DialogHeader>
        <div className="space-y-5">
          {shortcutGroups.map((group) => (
            <section key={group.label}>
              <h3 className="mb-1.5 text-xs font-medium text-neutral-400">
                {group.label}
              </h3>
              <dl className="divide-y divide-neutral-100">
                {group.items.map(([label, shortcut]) => (
                  <div
                    key={label}
                    className="flex items-center justify-between gap-4 py-2 text-sm"
                  >
                    <dt className="text-neutral-700">{label}</dt>
                    <dd>
                      <kbd className="inline-flex min-w-10 justify-center rounded border border-neutral-200 bg-neutral-50 px-1.5 py-0.5 font-mono text-xs text-neutral-600 shadow-sm">
                        {shortcut}
                      </kbd>
                    </dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </div>
        <DialogFooter>
          <button
            type="button"
            onClick={() => setOpenDialog(null)}
            className="rounded-md bg-neutral-900 px-3 py-1.5 text-sm text-white hover:bg-neutral-700"
          >
            閉じる
          </button>
        </DialogFooter>
      </Dialog>

      <Dialog
        open={openDialog === "about"}
        onOpenChange={(open) => {
          if (!open) setOpenDialog(null);
        }}
      >
        <div className="flex flex-col items-center py-2 text-center">
          <img
            src={appIconUrl}
            alt=""
            className="mb-4 size-14 object-contain"
          />
          <DialogHeader className="mb-2">
            <DialogTitle className="text-2xl">Track</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-neutral-500">
            シンプルな工数管理アプリ
          </p>
          <p className="mt-3 text-xs tabular-nums text-neutral-400">
            バージョン {packageJson.version}
          </p>
          <a
            href={REPOSITORY_URL}
            target="_blank"
            rel="noreferrer"
            onClick={(event) => {
              if (!("__TAURI_INTERNALS__" in window)) return;
              event.preventDefault();
              void openUrl(REPOSITORY_URL);
            }}
            className="mt-4 inline-flex items-center gap-1.5 text-sm text-neutral-500 underline-offset-4 hover:text-neutral-900 hover:underline focus-visible:rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-neutral-400"
          >
            <ExternalLink className="size-4" />
            GitHub
          </a>
        </div>
        <DialogFooter className="justify-center">
          <button
            type="button"
            onClick={() => setOpenDialog(null)}
            className="rounded-md bg-neutral-900 px-4 py-1.5 text-sm text-white hover:bg-neutral-700"
          >
            閉じる
          </button>
        </DialogFooter>
      </Dialog>
    </header>
  );
}
