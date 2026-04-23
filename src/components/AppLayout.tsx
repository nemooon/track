import { Link, Outlet, useLocation, useNavigate } from "react-router";
import { CalendarDays, BarChart3, FolderKanban, LogOut, UserCog } from "lucide-react";
import { cn } from "@/lib/utils";
import { useAuth } from "@/lib/auth";
import { useFaviconStatus } from "@/lib/useFaviconStatus";

const links = [
  { href: "/calendar", label: "カレンダー", icon: CalendarDays },
  { href: "/reports", label: "レポート", icon: BarChart3 },
  { href: "/settings/projects", label: "プロジェクト", icon: FolderKanban },
  { href: "/settings/account", label: "アカウント", icon: UserCog },
];

export function AppLayout() {
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const { user, signOut } = useAuth();
  useFaviconStatus();

  return (
    <div className="flex h-svh">
      <aside className="flex h-svh w-56 shrink-0 flex-col border-r border-neutral-200 bg-neutral-50 px-3 py-4">
        <div className="mb-6 px-2">
          <Link to="/calendar" className="text-lg font-semibold tracking-tight">
            track
          </Link>
        </div>
        <nav className="flex flex-col gap-1">
          {links.map(({ href, label, icon: Icon }) => {
            const active = pathname === href || pathname.startsWith(`${href}/`);
            return (
              <Link
                key={href}
                to={href}
                className={cn(
                  "flex items-center gap-2 rounded-md px-3 py-2 text-sm",
                  active
                    ? "bg-neutral-200 font-medium"
                    : "text-neutral-700 hover:bg-neutral-100",
                )}
              >
                <Icon className="h-4 w-4" />
                {label}
              </Link>
            );
          })}
        </nav>
        <div className="mt-auto space-y-2 border-t border-neutral-200 pt-3">
          <div className="px-2 text-xs text-neutral-500">{user?.email ?? ""}</div>
          <button
            onClick={async () => {
              await signOut();
              navigate("/login");
            }}
            className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm text-neutral-700 hover:bg-neutral-100"
          >
            <LogOut className="h-4 w-4" />
            ログアウト
          </button>
        </div>
      </aside>
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
