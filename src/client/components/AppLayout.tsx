import { Outlet } from "react-router";
import { AppHeader } from "@client/components/AppHeader";
import { useAppUi } from "@client/components/AppUiContext";
import { SettingsOverlay } from "@client/components/SettingsOverlay";
import { useFaviconStatus } from "@client/lib/useFaviconStatus";

export function AppLayout() {
  const { settingsOpen } = useAppUi();
  useFaviconStatus();

  return (
    <div className="relative flex h-svh flex-col">
      <AppHeader />
      <main className="min-h-0 flex-1 overflow-auto">
        <Outlet />
      </main>
      {settingsOpen && <SettingsOverlay />}
    </div>
  );
}
