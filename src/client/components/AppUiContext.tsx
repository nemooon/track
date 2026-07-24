import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useLocation, useNavigate } from "react-router";

interface AppUiContextValue {
  openSettings: () => void;
  closeSettings: () => boolean;
  settingsDirty: boolean;
  setSettingsDirty: (dirty: boolean) => void;
  confirmDiscardChanges: () => boolean;
}

const AppUiContext = createContext<AppUiContextValue | null>(null);

export function AppUiProvider({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const lastViewRef = useRef("/calendar");
  const [settingsDirty, setSettingsDirty] = useState(false);

  useEffect(() => {
    if (pathname === "/calendar" || pathname === "/reports") {
      lastViewRef.current = pathname;
    }
  }, [pathname]);

  const openSettings = useCallback(() => {
    if (!pathname.startsWith("/settings")) {
      navigate("/settings/work-hours");
    }
  }, [navigate, pathname]);
  const confirmDiscardChanges = useCallback(() => {
    if (!settingsDirty) return true;
    return window.confirm("保存していない変更があります。破棄して移動しますか？");
  }, [settingsDirty]);
  const closeSettings = useCallback(() => {
    if (!confirmDiscardChanges()) return false;
    setSettingsDirty(false);
    navigate(lastViewRef.current);
    return true;
  }, [confirmDiscardChanges, navigate]);

  useEffect(() => {
    if (!settingsDirty) return;
    const onBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, [settingsDirty]);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;

    let active = true;
    let unlisten: UnlistenFn | undefined;
    void listen("track-open-settings", openSettings).then(
      (dispose) => {
        if (active) unlisten = dispose;
        else dispose();
      },
    );

    return () => {
      active = false;
      unlisten?.();
    };
  }, [openSettings]);

  const value = useMemo<AppUiContextValue>(
    () => ({
      openSettings,
      closeSettings,
      settingsDirty,
      setSettingsDirty,
      confirmDiscardChanges,
    }),
    [
      closeSettings,
      confirmDiscardChanges,
      openSettings,
      settingsDirty,
    ],
  );

  return <AppUiContext.Provider value={value}>{children}</AppUiContext.Provider>;
}

export function useAppUi() {
  const context = useContext(AppUiContext);
  if (!context) {
    throw new Error("useAppUi must be used within AppUiProvider");
  }
  return context;
}
