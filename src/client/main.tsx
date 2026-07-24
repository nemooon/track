import { StrictMode, Suspense, lazy, useEffect } from "react";
import { createRoot } from "react-dom/client";
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  useNavigate,
} from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { AppLayout } from "@client/components/AppLayout";
import {
  AppUiProvider,
  useAppUi,
} from "@client/components/AppUiContext";
import { SettingsLayout } from "@client/components/SettingsLayout";
import { SettingsPage } from "@client/pages/SettingsPage";
import "./index.css";

const CalendarPage = lazy(() =>
  import("@client/pages/CalendarPage").then((m) => ({ default: m.CalendarPage })),
);
const ReportsPage = lazy(() =>
  import("@client/pages/ReportsPage").then((m) => ({ default: m.ReportsPage })),
);
const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000 } },
});

function AppNavigationShortcuts() {
  const navigate = useNavigate();
  const { closeSettings, openSettings } = useAppUi();

  useEffect(() => {
    function openView(path: "/calendar" | "/reports") {
      if (!closeSettings()) return;
      navigate(path);
    }

    function onKeyDown(event: KeyboardEvent) {
      if (
        event.defaultPrevented ||
        event.repeat ||
        event.altKey ||
        event.shiftKey ||
        !(event.metaKey || event.ctrlKey)
      ) {
        return;
      }

      if (event.key === "1") {
        event.preventDefault();
        openView("/calendar");
      } else if (event.key === "2") {
        event.preventDefault();
        openView("/reports");
      } else if (event.key === ",") {
        event.preventDefault();
        openSettings();
      }
    }

    window.addEventListener("keydown", onKeyDown);

    let active = true;
    let unlisten: UnlistenFn | undefined;
    if ("__TAURI_INTERNALS__" in window) {
      void listen<string>("track-open-view", ({ payload }) => {
        if (payload === "/calendar" || payload === "/reports") {
          openView(payload);
        }
      }).then((dispose) => {
        if (active) unlisten = dispose;
        else dispose();
      });
    }

    return () => {
      active = false;
      window.removeEventListener("keydown", onKeyDown);
      unlisten?.();
    };
  }, [closeSettings, navigate, openSettings]);

  return null;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <AppUiProvider>
          <AppNavigationShortcuts />
          <Suspense fallback={null}>
            <Routes>
              <Route element={<AppLayout />}>
                <Route path="/calendar" element={<CalendarPage />} />
                <Route path="/reports" element={<ReportsPage />} />
                <Route path="/settings" element={<SettingsLayout />}>
                  <Route index element={<Navigate to="work-hours" replace />} />
                  <Route
                    path="work-hours"
                    element={<SettingsPage category="work-hours" />}
                  />
                  <Route
                    path="weekly-report"
                    element={<SettingsPage category="weekly-report" />}
                  />
                  <Route
                    path="projects"
                    element={<SettingsPage category="projects" />}
                  />
                  <Route
                    path="backup"
                    element={<SettingsPage category="backup" />}
                  />
                  <Route
                    path="data-transfer"
                    element={<SettingsPage category="data-transfer" />}
                  />
                </Route>
              </Route>
              <Route path="*" element={<Navigate to="/calendar" replace />} />
            </Routes>
          </Suspense>
          <Toaster />
        </AppUiProvider>
      </BrowserRouter>
    </QueryClientProvider>
  </StrictMode>,
);
