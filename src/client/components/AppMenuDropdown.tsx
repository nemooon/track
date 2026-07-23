import { useEffect, useRef, useState } from "react";
import { Ellipsis, Settings } from "lucide-react";
import { useAppUi } from "@client/components/AppUiContext";
import { HeaderControlButton } from "@client/components/HeaderControls";

export function AppMenuDropdown() {
  const { openSettings } = useAppUi();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;

    function onPointerDown(event: MouseEvent) {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }

    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <HeaderControlButton
        onClick={() => setOpen((current) => !current)}
        aria-label="アプリメニュー"
        aria-haspopup="menu"
        aria-expanded={open}
        active={open}
        iconOnly
      >
        <Ellipsis className="size-5" />
      </HeaderControlButton>

      {open && (
        <div
          role="menu"
          className="absolute right-0 top-full z-50 mt-1 min-w-44 rounded-lg border border-neutral-200 bg-white p-1 shadow-lg"
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              openSettings();
            }}
            className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm text-neutral-700 hover:bg-neutral-100"
          >
            <Settings className="size-4 text-neutral-500" />
            <span>設定…</span>
            <kbd className="ml-auto text-xs text-neutral-400">⌘,</kbd>
          </button>
        </div>
      )}
    </div>
  );
}
