import { useEffect, useState } from "react";
import { Link, Outlet, useLocation } from "react-router";
import {
  CalendarDays,
  BarChart3,
  FolderKanban,
  Settings,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useFaviconStatus } from "@/lib/useFaviconStatus";
import { useMediaQuery } from "@/lib/useMediaQuery";

const links = [
  { href: "/calendar", label: "カレンダー", icon: CalendarDays },
  { href: "/reports", label: "レポート", icon: BarChart3 },
  { href: "/settings/projects", label: "プロジェクト", icon: FolderKanban },
  { href: "/settings", label: "設定", icon: Settings },
];

const SIDEBAR_STORAGE_KEY = "track:sidebar-collapsed";

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(SIDEBAR_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function AppLayout() {
  const { pathname } = useLocation();
  useFaviconStatus();
  const isMobile = useMediaQuery("(max-width: 639px)");
  const [storedCollapsed, setStoredCollapsed] = useState<boolean>(() => readCollapsed());
  const collapsed = isMobile ? true : storedCollapsed;

  useEffect(() => {
    if (isMobile) return;
    try {
      localStorage.setItem(SIDEBAR_STORAGE_KEY, storedCollapsed ? "1" : "0");
    } catch {
      // ignore
    }
  }, [storedCollapsed, isMobile]);

  return (
    <div className="flex h-svh">
      <aside
        className={cn(
          "flex h-svh shrink-0 flex-col border-r border-neutral-200 bg-neutral-50 py-4 transition-[width] duration-200",
          collapsed ? "w-14 px-1.5" : "w-56 px-3",
        )}
      >
        <div
          className={cn(
            "mb-6 flex items-center",
            collapsed ? "justify-center" : "justify-between px-2",
          )}
        >
          {!collapsed && (
            <Link to="/calendar" className="text-lg font-semibold tracking-tight">
              track
            </Link>
          )}
          <button
            type="button"
            onClick={() => setStoredCollapsed((v) => !v)}
            aria-label={collapsed ? "サイドバーを開く" : "サイドバーを折り畳む"}
            title={collapsed ? "サイドバーを開く" : "サイドバーを折り畳む"}
            disabled={isMobile}
            className="flex h-8 w-8 items-center justify-center rounded-md text-neutral-600 hover:bg-neutral-200 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
          >
            {collapsed ? (
              <PanelLeftOpen className="h-4 w-4" />
            ) : (
              <PanelLeftClose className="h-4 w-4" />
            )}
          </button>
        </div>
        <nav className="flex flex-col gap-1">
          {links.map(({ href, label, icon: Icon }) => {
            const active = pathname === href || pathname.startsWith(`${href}/`);
            return (
              <Link
                key={href}
                to={href}
                title={collapsed ? label : undefined}
                className={cn(
                  "flex items-center rounded-md py-2 text-sm",
                  collapsed ? "justify-center px-0" : "gap-2 px-3",
                  active
                    ? "bg-neutral-200 font-medium"
                    : "text-neutral-700 hover:bg-neutral-100",
                )}
              >
                <Icon className="h-4 w-4 shrink-0" />
                {!collapsed && label}
              </Link>
            );
          })}
        </nav>
      </aside>
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
