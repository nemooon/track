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

function OpenSettingsRoute() {
  const navigate = useNavigate();
  const { openSettings } = useAppUi();

  useEffect(() => {
    openSettings();
    navigate("/calendar", { replace: true });
  }, [navigate, openSettings]);

  return null;
}

function AppNavigationShortcuts() {
  const navigate = useNavigate();
  const { closeSettings } = useAppUi();

  useEffect(() => {
    function openView(path: "/calendar" | "/reports") {
      closeSettings();
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
  }, [closeSettings, navigate]);

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
                <Route path="/settings/projects" element={<OpenSettingsRoute />} />
                <Route path="/settings" element={<OpenSettingsRoute />} />
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
