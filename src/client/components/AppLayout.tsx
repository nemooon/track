import { Outlet, useLocation } from "react-router";
import { AppHeader } from "@client/components/AppHeader";
import { useAppUi } from "@client/components/AppUiContext";
import { SettingsOverlay } from "@client/components/SettingsOverlay";
import { useFaviconStatus } from "@client/lib/useFaviconStatus";

export function AppLayout() {
  const { pathname } = useLocation();
  const { settingsOpen } = useAppUi();
  useFaviconStatus();
  const isTauri = "__TAURI_INTERNALS__" in window;
  const hasPageHeader = pathname === "/calendar" || pathname === "/reports";

  return (
    <div className="relative flex h-svh flex-col">
      {hasPageHeader ? (
        <AppHeader />
      ) : (
        isTauri && (
          <div data-tauri-drag-region className="h-14 shrink-0" />
        )
      )}
      <main className="min-h-0 flex-1 overflow-auto">
        <Outlet />
      </main>
      {settingsOpen && <SettingsOverlay />}
    </div>
  );
}
