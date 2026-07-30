import { useRef } from "react";
import { Outlet } from "react-router";
import { AppHeader } from "@client/components/AppHeader";
import { NativeTitleTooltipArea } from "@client/components/ui/tooltip";
import { useFaviconStatus } from "@client/lib/useFaviconStatus";

export function AppLayout() {
  const rootRef = useRef<HTMLDivElement>(null);
  useFaviconStatus();

  return (
    <div ref={rootRef} className="relative flex h-svh flex-col">
      <AppHeader />
      <main className="min-h-0 flex-1 overflow-auto">
        <Outlet />
      </main>
      <NativeTitleTooltipArea rootRef={rootRef} />
    </div>
  );
}
