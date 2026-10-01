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
import { ServerChangeListener } from "@client/components/ServerChangeListener";
import "./index.css";

const CalendarPage = lazy(() =>
  import("@client/pages/CalendarPage").then((m) => ({ default: m.CalendarPage })),
);
const ReportsPage = lazy(() =>
  import("@client/pages/ReportsPage").then((m) => ({ default: m.ReportsPage })),
);
const NotesPage = lazy(() =>
  import("@client/pages/NotesPage").then((m) => ({ default: m.NotesPage })),
);
const ReportCopyFormatPage = lazy(() =>
  import("@client/pages/ReportCopyFormatPage").then((m) => ({
    default: m.ReportCopyFormatPage,
  })),
);
const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000 } },
});

function AppNavigationShortcuts() {
  const navigate = useNavigate();
  const { closeSettings, openSettings } = useAppUi();

  useEffect(() => {
    function openView(path: "/calendar" | "/reports" | "/notes") {
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
      } else if (event.key === "3") {
        event.preventDefault();
        openView("/notes");
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
        if (
          payload === "/calendar" ||
          payload === "/reports" ||
          payload === "/notes"
        ) {
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
      <ServerChangeListener />
      <BrowserRouter>
        <AppUiProvider>
          <AppNavigationShortcuts />
          <Suspense fallback={null}>
            <Routes>
              <Route element={<AppLayout />}>
                <Route path="/calendar" element={<CalendarPage />} />
                <Route path="/reports" element={<ReportsPage />} />
                <Route path="/notes" element={<NotesPage />} />
              </Route>
              <Route
                path="/report-formats/:formatId"
                element={<ReportCopyFormatPage />}
              />
              <Route path="/settings" element={<SettingsLayout />}>
                <Route index element={<Navigate to="calendar" replace />} />
                <Route
                  path="calendar"
                  element={<SettingsPage category="calendar" />}
                />
                <Route
                  path="reports"
                  element={<SettingsPage category="reports" />}
                />
                <Route
                  path="notes"
                  element={<SettingsPage category="notes" />}
                />
                <Route
                  path="projects"
                  element={<SettingsPage category="projects" />}
                />
                <Route
                  path="ai"
                  element={<SettingsPage category="ai" />}
                />
                <Route
                  path="backup"
                  element={<SettingsPage category="backup" />}
                />
                <Route
                  path="work-hours"
                  element={<Navigate to="../calendar" replace />}
                />
                <Route
                  path="weekly-report"
                  element={<Navigate to="../reports" replace />}
                />
                <Route
                  path="data-transfer"
                  element={<Navigate to="../backup" replace />}
                />
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
