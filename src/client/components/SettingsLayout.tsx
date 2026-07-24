import {
  ArrowLeftRight,
  BriefcaseBusiness,
  Clock3,
  DatabaseBackup,
  X,
} from "lucide-react";
import { NavLink, Outlet, useLocation } from "react-router";
import { useAppUi } from "@client/components/AppUiContext";
import { cn } from "@client/lib/utils";

const categories = [
  {
    href: "/settings/work-hours",
    label: "勤務時間",
    icon: Clock3,
  },
  {
    href: "/settings/projects",
    label: "プロジェクト管理",
    icon: BriefcaseBusiness,
  },
  {
    href: "/settings/backup",
    label: "バックアップと復元",
    icon: DatabaseBackup,
  },
  {
    href: "/settings/data-transfer",
    label: "データ移行",
    icon: ArrowLeftRight,
  },
];

export function SettingsLayout() {
  const { pathname } = useLocation();
  const {
    closeSettings,
    confirmDiscardChanges,
    setSettingsDirty,
  } = useAppUi();

  return (
    <div className="flex min-h-full bg-white sm:h-full sm:min-h-0">
      <aside className="flex w-full shrink-0 flex-col border-b border-neutral-200 bg-neutral-50 sm:w-60 sm:border-b-0 sm:border-r">
        <header className="flex h-14 shrink-0 items-center justify-between px-4">
          <h1 className="text-lg font-semibold tracking-tight">設定</h1>
          <button
            type="button"
            onClick={closeSettings}
            aria-label="設定を閉じる"
            className="flex size-8 items-center justify-center rounded-md text-neutral-500 transition-colors hover:bg-neutral-200/70 hover:text-neutral-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-neutral-400"
          >
            <X className="size-4" />
          </button>
        </header>

        <nav
          aria-label="設定カテゴリ"
          className="flex gap-1 overflow-x-auto px-2 pb-2 sm:flex-col sm:overflow-visible sm:pb-0"
        >
          {categories.map(({ href, label, icon: Icon }) => (
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
        </nav>
      </aside>

      <section className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-3xl p-5 sm:p-8">
          <Outlet />
        </div>
      </section>
    </div>
  );
}
