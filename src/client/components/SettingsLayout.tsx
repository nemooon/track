import {
  BarChart3,
  BriefcaseBusiness,
  CalendarDays,
  DatabaseBackup,
  NotebookPen,
  Sparkles,
  X,
} from "lucide-react";
import { NavLink, Outlet, useLocation } from "react-router";
import { useAppUi } from "@client/components/AppUiContext";
import { cn } from "@client/lib/utils";

const categoryGroups = [
  {
    label: "ビュー",
    categories: [
      {
        href: "/settings/calendar",
        label: "カレンダー",
        icon: CalendarDays,
      },
      {
        href: "/settings/reports",
        label: "レポート",
        icon: BarChart3,
      },
      {
        href: "/settings/notes",
        label: "メモ",
        icon: NotebookPen,
      },
    ],
  },
  {
    label: "共通",
    categories: [
      {
        href: "/settings/projects",
        label: "プロジェクト",
        icon: BriefcaseBusiness,
      },
      {
        href: "/settings/ai",
        label: "AI",
        icon: Sparkles,
      },
    ],
  },
  {
    label: "データ",
    categories: [
      {
        href: "/settings/backup",
        label: "バックアップと復元",
        icon: DatabaseBackup,
      },
    ],
  },
];

export function SettingsLayout() {
  const { pathname } = useLocation();
  const isTauri = "__TAURI_INTERNALS__" in window;
  const activeCategory = categoryGroups
    .flatMap((group) => group.categories)
    .find((category) => category.href === pathname);
  const {
    closeSettings,
    confirmDiscardChanges,
    setSettingsDirty,
  } = useAppUi();

  return (
    <div className="relative flex h-svh min-h-0 flex-col overflow-hidden bg-white sm:flex-row">
      <aside className="flex w-full shrink-0 flex-col border-b border-neutral-200 bg-neutral-50 sm:w-64 sm:border-b-0 sm:border-r">
        <header
          data-tauri-drag-region={isTauri ? "" : undefined}
          className={cn(
            "flex h-11 shrink-0 items-center pr-4",
            isTauri ? "pl-[92px]" : "pl-4",
          )}
        >
          <h1 className="pointer-events-none text-sm font-semibold text-neutral-950">
            設定
          </h1>
        </header>

        <nav
          aria-label="設定カテゴリ"
          className="subtle-scrollbar flex gap-3 overflow-x-auto px-2 pb-2 sm:flex-col sm:gap-5 sm:overflow-y-auto sm:px-3 sm:pb-5"
        >
          {categoryGroups.map((group) => (
            <div key={group.label} className="flex shrink-0 gap-1 sm:flex-col">
              <div className="hidden px-3 pb-1 text-[11px] font-medium uppercase tracking-wider text-neutral-400 sm:block">
                {group.label}
              </div>
              {group.categories.map(({ href, label, icon: Icon }) => (
                <NavLink
                  key={href}
                  to={href}
                  onClick={(event) => {
                    if (pathname !== href && !confirmDiscardChanges()) {
                      event.preventDefault();
                      return;
                    }
                    if (pathname !== href) setSettingsDirty(false);
                  }}
                  className={({ isActive }) =>
                    cn(
                      "flex shrink-0 items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-neutral-400",
                      isActive
                        ? "bg-white text-neutral-950 shadow-sm ring-1 ring-neutral-200"
                        : "text-neutral-600 hover:bg-neutral-200/70 hover:text-neutral-950",
                    )
                  }
                >
                  <Icon className="size-4" />
                  {label}
                </NavLink>
              ))}
            </div>
          ))}
        </nav>
      </aside>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-white">
        <header
          data-tauri-drag-region={isTauri ? "" : undefined}
          className="flex h-11 shrink-0 items-center gap-3 border-b border-neutral-200 bg-white px-4"
        >
          <h2 className="pointer-events-none min-w-0 flex-1 truncate text-sm font-semibold text-neutral-950">
            {activeCategory?.label ?? "設定"}
          </h2>
          <button
            type="button"
            onClick={closeSettings}
            aria-label="設定を閉じる"
            className="flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-500 transition-colors hover:bg-neutral-100 hover:text-neutral-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-neutral-400"
          >
            <X className="size-4" />
          </button>
        </header>
        <section className="subtle-scrollbar min-h-0 min-w-0 flex-1 overflow-y-auto">
          <div className="mx-auto w-full max-w-7xl p-5 sm:p-8 lg:p-10">
            <Outlet />
          </div>
        </section>
        <footer
          id="settings-actions-root"
          className="empty:hidden shrink-0 border-t border-neutral-200 bg-white/95 shadow-[0_-8px_24px_rgba(0,0,0,0.035)] backdrop-blur"
        />
      </div>
    </div>
  );
}
