import { useEffect, useState } from "react";
import { Link, Outlet, useLocation, useNavigate } from "react-router";
import {
  CalendarDays,
  BarChart3,
  FolderKanban,
  LogOut,
  UserCog,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useAuth } from "@/lib/auth";
import { useFaviconStatus } from "@/lib/useFaviconStatus";

const links = [
  { href: "/calendar", label: "カレンダー", icon: CalendarDays },
  { href: "/reports", label: "レポート", icon: BarChart3 },
  { href: "/settings/projects", label: "プロジェクト", icon: FolderKanban },
  { href: "/settings/account", label: "アカウント", icon: UserCog },
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
  const navigate = useNavigate();
  const { user, signOut } = useAuth();
  useFaviconStatus();
  const [collapsed, setCollapsed] = useState<boolean>(() => readCollapsed());

  useEffect(() => {
    try {
      localStorage.setItem(SIDEBAR_STORAGE_KEY, collapsed ? "1" : "0");
    } catch {
      // ignore
    }
  }, [collapsed]);

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
            onClick={() => setCollapsed((v) => !v)}
            aria-label={collapsed ? "サイドバーを開く" : "サイドバーを折り畳む"}
            title={collapsed ? "サイドバーを開く" : "サイドバーを折り畳む"}
            className="flex h-8 w-8 items-center justify-center rounded-md text-neutral-600 hover:bg-neutral-200"
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
        <div
          className={cn(
            "mt-auto border-t border-neutral-200 pt-3",
            collapsed ? "space-y-1" : "space-y-2",
          )}
        >
          {!collapsed && (
            <div className="truncate px-2 text-xs text-neutral-500">
              {user?.email ?? ""}
            </div>
          )}
          <button
            onClick={async () => {
              await signOut();
              navigate("/login");
            }}
            title={collapsed ? "ログアウト" : undefined}
            className={cn(
              "flex w-full items-center rounded-md py-2 text-sm text-neutral-700 hover:bg-neutral-100",
              collapsed ? "justify-center px-0" : "gap-2 px-3",
            )}
          >
            <LogOut className="h-4 w-4 shrink-0" />
            {!collapsed && "ログアウト"}
          </button>
        </div>
      </aside>
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
