import { Outlet } from "react-router";
import { AppHeader } from "@client/components/AppHeader";
import { useFaviconStatus } from "@client/lib/useFaviconStatus";

export function AppLayout() {
  useFaviconStatus();

  return (
    <div className="relative flex h-svh flex-col">
      <AppHeader />
      <main className="min-h-0 flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
