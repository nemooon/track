import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface AppUiContextValue {
  settingsOpen: boolean;
  openSettings: () => void;
  closeSettings: () => void;
}

const AppUiContext = createContext<AppUiContextValue | null>(null);

export function AppUiProvider({ children }: { children: ReactNode }) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const openSettings = useCallback(() => setSettingsOpen(true), []);
  const closeSettings = useCallback(() => setSettingsOpen(false), []);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;

    let active = true;
    let unlisten: UnlistenFn | undefined;
    void listen("track-open-settings", () => setSettingsOpen(true)).then(
      (dispose) => {
        if (active) unlisten = dispose;
        else dispose();
      },
    );

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const value = useMemo<AppUiContextValue>(
    () => ({
      settingsOpen,
      openSettings,
      closeSettings,
    }),
    [closeSettings, openSettings, settingsOpen],
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
