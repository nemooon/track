import { CalendarDays, BarChart3 } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Link, useLocation } from "react-router";
import { AppMenuDropdown } from "@client/components/AppMenuDropdown";
import {
  headerControlGroupClass,
  headerControlItemClass,
} from "@client/components/HeaderControls";

const views = [
  { href: "/calendar", label: "カレンダー", icon: CalendarDays },
  { href: "/reports", label: "レポート", icon: BarChart3 },
];

export function AppHeader() {
  const { pathname } = useLocation();

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
      className="flex h-14 shrink-0 items-center gap-2 border-b border-[#1d2824] bg-[#2e3a35] pl-[92px] pr-3"
    >
      <div className="flex shrink-0 items-center gap-1">
        <nav className={headerControlGroupClass}>
          {views.map(({ href, label, icon: Icon }) => {
            const active = pathname === href;
            return (
              <Link
                key={href}
                to={href}
                title={`${label}（⌘${href === "/calendar" ? "1" : "2"}）`}
                aria-keyshortcuts={`Meta+${href === "/calendar" ? "1" : "2"}`}
                className={headerControlItemClass(active)}
              >
                <Icon className="size-4" />
                {label}
              </Link>
            );
          })}
        </nav>
      </div>
      <div
        id="page-header-center"
        className="flex min-w-0 flex-1 items-center justify-center gap-1 overflow-x-auto"
      />
      <div className="flex shrink-0 items-center gap-1">
        <div
          id="page-header-right"
          className="flex shrink-0 items-center gap-2"
        />
        <AppMenuDropdown />
      </div>
    </header>
  );
}
