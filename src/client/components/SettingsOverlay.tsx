import { useEffect } from "react";
import { X } from "lucide-react";
import { useAppUi } from "@client/components/AppUiContext";
import { SettingsPage } from "@client/pages/SettingsPage";

export function SettingsOverlay() {
  const { closeSettings } = useAppUi();

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") closeSettings();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [closeSettings]);

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center bg-neutral-950/20 p-4 pt-16 backdrop-blur-[2px]"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) closeSettings();
      }}
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        className="flex max-h-full w-full max-w-4xl flex-col overflow-hidden rounded-xl border border-neutral-200 bg-white shadow-2xl"
      >
        <header className="flex h-12 shrink-0 items-center justify-between border-b border-neutral-200 px-4">
          <h1 id="settings-title" className="text-base font-semibold">
            設定
          </h1>
          <button
            type="button"
            onClick={closeSettings}
            aria-label="設定を閉じる"
            className="flex size-8 items-center justify-center rounded-md text-neutral-500 hover:bg-neutral-100 hover:text-neutral-900"
          >
            <X className="size-4" />
          </button>
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto">
          <SettingsPage embedded />
        </div>
      </section>
    </div>
  );
}
