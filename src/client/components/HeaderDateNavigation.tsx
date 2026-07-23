import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  DateRangeNavigator,
  type DateRange,
} from "@client/components/ui/DateRangeNavigator";

export function HeaderDateNavigation({
  anchor,
  range,
  onPrev,
  onNext,
  onAnchorChange,
  onToday,
}: {
  anchor: Date;
  range: DateRange;
  onPrev: () => void;
  onNext: () => void;
  onAnchorChange: (next: Date) => void;
  onToday: () => void;
}) {
  useEffect(() => {
    function perform(action: "previous" | "next" | "today") {
      if (action === "previous") onPrev();
      else if (action === "next") onNext();
      else onToday();
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

      const action =
        event.code === "BracketLeft"
          ? "previous"
          : event.code === "BracketRight"
            ? "next"
            : event.code === "KeyT"
              ? "today"
              : null;
      if (!action) return;

      event.preventDefault();
      perform(action);
    }

    const isTauri = "__TAURI_INTERNALS__" in window;
    if (!isTauri) window.addEventListener("keydown", onKeyDown);

    let active = true;
    let unlisten: UnlistenFn | undefined;
    if (isTauri) {
      void listen<string>("track-date-navigation", ({ payload }) => {
        if (
          payload === "previous" ||
          payload === "next" ||
          payload === "today"
        ) {
          perform(payload);
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
  }, [onNext, onPrev, onToday]);

  return (
    <>
      <DateRangeNavigator
        anchor={anchor}
        range={range}
        onPrev={onPrev}
        onNext={onNext}
        onAnchorChange={onAnchorChange}
        onToday={onToday}
      />
    </>
  );
}
