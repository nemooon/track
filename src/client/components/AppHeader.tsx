import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowLeftRight,
  BarChart3,
  Bot,
  CalendarDays,
  Ellipsis,
  ExternalLink,
  Info,
  Keyboard,
  NotebookPen,
  RefreshCw,
  Settings,
} from "lucide-react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Link, useLocation } from "react-router";
import { toast } from "sonner";
import { useAppUi } from "@client/components/AppUiContext";
import { DataTransferDialog } from "@client/components/DataTransferDialog";
import {
  Dialog,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@client/components/ui/dialog";
import { cn } from "@client/lib/utils";
import { findAvailableUpdate } from "@client/lib/appUpdate";
import packageJson from "../../../package.json";
import appIconUrl from "../../../src-tauri/icons/128x128.png";

const REPOSITORY_URL = "https://github.com/nemooon/track";
const UPDATE_CHECK_INTERVAL = 24 * 60 * 60 * 1000;
const UPDATE_COMMAND =
  "brew upgrade --cask --no-quit --no-ask nemooon/tap/track";

const views = [
  { href: "/calendar", label: "カレンダー", icon: CalendarDays, shortcut: "1" },
  { href: "/reports", label: "レポート", icon: BarChart3, shortcut: "2" },
  { href: "/notes", label: "メモ", icon: NotebookPen, shortcut: "3" },
];

const shortcutGroups = [
  {
    label: "画面",
    items: [
      ["カレンダーを開く", "⌘1"],
      ["レポートを開く", "⌘2"],
      ["メモを開く", "⌘3"],
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
  const { pathname, search } = useLocation();
  const {
    confirmDiscardChanges,
    openSettings,
    setSettingsDirty,
  } = useAppUi();
  const updatePreviewVersion = import.meta.env.DEV
    ? new URLSearchParams(search).get("trackUpdateVersion")
    : null;
  const [menuOpen, setMenuOpen] = useState(false);
  const [openDialog, setOpenDialog] = useState<
    "shortcuts" | "data-transfer" | "about" | "update" | null
  >(null);
  const [availableVersion, setAvailableVersion] = useState<string | null>(null);
  const [isCheckingForUpdates, setIsCheckingForUpdates] = useState(false);
  const [isUpdating, setIsUpdating] = useState(false);
  const [updateInstalled, setUpdateInstalled] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [reserveTrafficLightSpace, setReserveTrafficLightSpace] = useState(
    "__TAURI_INTERNALS__" in window,
  );
  const menuRef = useRef<HTMLDivElement>(null);

  const checkForUpdates = useCallback(async (announceResult = false) => {
    setIsCheckingForUpdates(true);
    try {
      const version =
        updatePreviewVersion ??
        (await findAvailableUpdate(packageJson.version));
      setAvailableVersion(version);
      if (announceResult) {
        if (version) {
          toast.info(`Trackの新しいバージョン ${version} があります`);
        } else {
          toast.success(`Track ${packageJson.version} は最新版です`);
        }
      }
    } catch (error) {
      console.info("アップデートを確認できませんでした", error);
      if (announceResult) {
        toast.error("アップデートを確認できませんでした");
      }
    } finally {
      setIsCheckingForUpdates(false);
    }
  }, [updatePreviewVersion]);

  useEffect(() => {
    if (updatePreviewVersion) {
      void checkForUpdates();
      return;
    }
    if (!("__TAURI_INTERNALS__" in window)) return;

    void checkForUpdates();
    const timer = window.setInterval(checkForUpdates, UPDATE_CHECK_INTERVAL);
    return () => window.clearInterval(timer);
  }, [checkForUpdates, updatePreviewVersion]);

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

    const appWindow = getCurrentWindow();
    let active = true;
    let unlisten: UnlistenFn | undefined;

    const syncTrafficLightSpace = async () => {
      const fullscreen = await appWindow.isFullscreen();
      if (active) setReserveTrafficLightSpace(!fullscreen);
    };

    void syncTrafficLightSpace();
    void appWindow.onResized(() => void syncTrafficLightSpace()).then(
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

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;

    let active = true;
    const unlisteners: UnlistenFn[] = [];
    void Promise.all([
      listen("track-open-about", () => setOpenDialog("about")),
      listen("track-check-for-updates", () => void checkForUpdates(true)),
      listen("track-open-data-transfer", () => setOpenDialog("data-transfer")),
    ]).then((disposers) => {
      if (active) unlisteners.push(...disposers);
      else disposers.forEach((dispose) => dispose());
    });

    return () => {
      active = false;
      unlisteners.forEach((dispose) => dispose());
    };
  }, [checkForUpdates]);

  function isWindowControlTarget(event: React.MouseEvent<HTMLElement>) {
    return (
      event.button === 0 &&
      "__TAURI_INTERNALS__" in window &&
      !(event.target as HTMLElement).closest(
        "a, button, input, select, textarea, [role='menu'], [role='dialog']",
      )
    );
  }

  function startWindowDrag(event: React.MouseEvent<HTMLElement>) {
    if (!isWindowControlTarget(event) || event.detail > 1) return;

    event.preventDefault();
    void getCurrentWindow().startDragging();
  }

  function toggleWindowMaximize(event: React.MouseEvent<HTMLElement>) {
    if (!isWindowControlTarget(event)) return;

    event.preventDefault();
    void getCurrentWindow().toggleMaximize();
  }

  async function installUpdate() {
    if (!availableVersion) return;
    setIsUpdating(true);
    setUpdateError(null);
    try {
      if (updatePreviewVersion) {
        await new Promise((resolve) => window.setTimeout(resolve, 800));
      } else {
        await invoke("install_update", { expectedVersion: availableVersion });
      }
      setUpdateInstalled(true);
    } catch (error) {
      setUpdateError(String(error));
    } finally {
      setIsUpdating(false);
    }
  }

  function restartTrack() {
    if (updatePreviewVersion) {
      setOpenDialog(null);
      toast.success("再起動処理を確認しました");
      return;
    }
    void invoke("restart_track");
  }

  return (
    <header
      onMouseDown={startWindowDrag}
      onDoubleClick={toggleWindowMaximize}
      className={cn(
        "relative flex h-11 shrink-0 items-center border-b border-[#1d2824] bg-[#2e3a35] pr-3",
        reserveTrafficLightSpace ? "pl-[92px]" : "pl-3",
      )}
    >
      <nav className="inline-flex h-full items-center gap-1">
        {views.map(({ href, label, icon: Icon, shortcut }) => {
          const active = pathname === href;
          return (
            <Link
              key={href}
              to={href}
              onClick={(event) => {
                if (pathname === href || confirmDiscardChanges()) {
                  setSettingsDirty(false);
                  return;
                }
                event.preventDefault();
              }}
              title={`${label}（⌘${shortcut}）`}
              aria-keyshortcuts={`Meta+${shortcut}`}
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

      <div
        ref={menuRef}
        className="relative ml-auto flex shrink-0 items-center gap-2"
      >
        {availableVersion && (
          <button
            type="button"
            onClick={() => {
              setUpdateError(null);
              setOpenDialog("update");
            }}
            className="whitespace-nowrap text-[11px] font-medium text-white/75 transition-colors hover:text-white focus-visible:rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/60"
          >
            新しいバージョンがあります
          </button>
        )}
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
                void checkForUpdates(true);
              }}
              disabled={isCheckingForUpdates}
              className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm text-neutral-700 hover:bg-neutral-100 disabled:cursor-wait disabled:text-neutral-400"
            >
              <RefreshCw
                className={cn(
                  "size-4 text-neutral-500",
                  isCheckingForUpdates && "animate-spin",
                )}
              />
              <span>
                {isCheckingForUpdates
                  ? "アップデートを確認中…"
                  : "アップデートを確認…"}
              </span>
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
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setMenuOpen(false);
                setOpenDialog("data-transfer");
              }}
              className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm text-neutral-700 hover:bg-neutral-100"
            >
              <ArrowLeftRight className="size-4 text-neutral-500" />
              <span>データの移行…</span>
            </button>
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

      <DataTransferDialog
        open={openDialog === "data-transfer"}
        onOpenChange={(open) => {
          if (!open) setOpenDialog(null);
        }}
      />

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
        open={openDialog === "update"}
        onOpenChange={(open) => {
          if (!open && !isUpdating) setOpenDialog(null);
        }}
      >
        <DialogHeader>
          <DialogTitle>Trackをアップデート</DialogTitle>
        </DialogHeader>
        {updateInstalled ? (
          <div className="space-y-2 text-sm text-neutral-600">
            <p>
              バージョン {availableVersion} のインストールが完了しました。
            </p>
            <p>再起動すると新しいバージョンへ切り替わります。</p>
          </div>
        ) : (
          <div className="space-y-4 text-sm text-neutral-600">
            <p>
              バージョン {packageJson.version} から {availableVersion}{" "}
              へアップデートします。
            </p>
            <div>
              <p className="mb-1.5 text-xs text-neutral-400">
                実行するコマンド
              </p>
              <code className="block overflow-x-auto rounded-md bg-neutral-100 px-3 py-2 text-xs text-neutral-700">
                {UPDATE_COMMAND}
              </code>
            </div>
            <p className="text-xs leading-5 text-neutral-400">
              Homebrewからダウンロードしている間もTrackはそのまま利用できます。
            </p>
            {updateError && (
              <div
                role="alert"
                className="whitespace-pre-wrap rounded-md bg-red-50 px-3 py-2 text-xs leading-5 text-red-700"
              >
                {updateError}
              </div>
            )}
          </div>
        )}
        <DialogFooter>
          <button
            type="button"
            onClick={() => setOpenDialog(null)}
            disabled={isUpdating}
            className="rounded-md border border-neutral-200 px-3 py-1.5 text-sm text-neutral-600 hover:bg-neutral-50 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {updateInstalled ? "あとで" : "キャンセル"}
          </button>
          {updateInstalled ? (
            <button
              type="button"
              data-dialog-autofocus
              onClick={restartTrack}
              className="rounded-md bg-neutral-900 px-3 py-1.5 text-sm text-white hover:bg-neutral-700"
            >
              再起動
            </button>
          ) : (
            <button
              type="button"
              data-dialog-autofocus
              onClick={() => void installUpdate()}
              disabled={isUpdating}
              className="rounded-md bg-neutral-900 px-3 py-1.5 text-sm text-white hover:bg-neutral-700 disabled:cursor-wait disabled:opacity-60"
            >
              {isUpdating ? "アップデート中…" : "アップデート"}
            </button>
          )}
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
