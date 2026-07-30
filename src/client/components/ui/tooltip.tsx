import * as React from "react";
import { createPortal } from "react-dom";

type TooltipChildProps = React.HTMLAttributes<HTMLElement> & {
  ref?: React.Ref<HTMLElement>;
  "aria-describedby"?: string;
};

export function Tooltip({
  content,
  children,
  delay = 250,
}: {
  content: React.ReactNode;
  children: React.ReactElement<TooltipChildProps>;
  delay?: number;
}) {
  const tooltipId = React.useId();
  const triggerRef = React.useRef<HTMLElement>(null);
  const tooltipRef = React.useRef<HTMLDivElement>(null);
  const openTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoveredRef = React.useRef(false);
  const focusedRef = React.useRef(false);
  const dismissedRef = React.useRef(false);
  const [mounted, setMounted] = React.useState(false);
  const [open, setOpen] = React.useState(false);
  const [position, setPosition] = React.useState<{
    left: number;
    top: number;
  } | null>(null);

  const clearOpenTimer = React.useCallback(() => {
    if (!openTimerRef.current) return;
    clearTimeout(openTimerRef.current);
    openTimerRef.current = null;
  }, []);

  const showAfterDelay = React.useCallback(() => {
    clearOpenTimer();
    openTimerRef.current = setTimeout(() => {
      setOpen(true);
      openTimerRef.current = null;
    }, delay);
  }, [clearOpenTimer, delay]);

  const closeIfInactive = React.useCallback(() => {
    clearOpenTimer();
    if (!hoveredRef.current && !focusedRef.current) setOpen(false);
  }, [clearOpenTimer]);

  const updatePosition = React.useCallback(() => {
    const trigger = triggerRef.current;
    const tooltip = tooltipRef.current;
    if (!trigger || !tooltip) return;

    const triggerRect = trigger.getBoundingClientRect();
    const tooltipRect = tooltip.getBoundingClientRect();
    const viewportPadding = 8;
    const gap = 8;
    const left = Math.min(
      window.innerWidth - tooltipRect.width - viewportPadding,
      Math.max(
        viewportPadding,
        triggerRect.left + triggerRect.width / 2 - tooltipRect.width / 2,
      ),
    );
    const top =
      triggerRect.top - tooltipRect.height - gap >= viewportPadding
        ? triggerRect.top - tooltipRect.height - gap
        : triggerRect.bottom + gap;
    setPosition({ left, top });
  }, []);

  React.useEffect(() => {
    setMounted(true);
    return clearOpenTimer;
  }, [clearOpenTimer]);

  React.useLayoutEffect(() => {
    if (!open) {
      setPosition(null);
      return;
    }

    updatePosition();
    const frame = requestAnimationFrame(updatePosition);
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [open, updatePosition]);

  React.useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [open]);

  const child = React.Children.only(children);
  const childProps = child.props;
  const trigger = React.cloneElement(child, {
    ref: triggerRef,
    "aria-describedby": open ? tooltipId : childProps["aria-describedby"],
    onMouseEnter: (event: React.MouseEvent<HTMLElement>) => {
      childProps.onMouseEnter?.(event);
      hoveredRef.current = true;
      dismissedRef.current = false;
      showAfterDelay();
    },
    onMouseLeave: (event: React.MouseEvent<HTMLElement>) => {
      childProps.onMouseLeave?.(event);
      hoveredRef.current = false;
      closeIfInactive();
    },
    onFocus: (event: React.FocusEvent<HTMLElement>) => {
      childProps.onFocus?.(event);
      focusedRef.current = true;
      if (dismissedRef.current) return;
      clearOpenTimer();
      setOpen(true);
    },
    onBlur: (event: React.FocusEvent<HTMLElement>) => {
      childProps.onBlur?.(event);
      focusedRef.current = false;
      closeIfInactive();
    },
    onPointerDown: (event: React.PointerEvent<HTMLElement>) => {
      childProps.onPointerDown?.(event);
      dismissedRef.current = true;
      clearOpenTimer();
      setOpen(false);
    },
  });

  return (
    <>
      {trigger}
      {mounted &&
        open &&
        createPortal(
          <div
            ref={tooltipRef}
            id={tooltipId}
            role="tooltip"
            style={{
              left: position?.left ?? 0,
              top: position?.top ?? 0,
              visibility: position ? "visible" : "hidden",
            }}
            className="pointer-events-none fixed z-[300] max-w-72 rounded-md bg-neutral-900 px-2.5 py-1.5 text-center text-xs leading-4 text-white shadow-lg"
          >
            {content}
          </div>,
          document.body,
        )}
    </>
  );
}

export function DelegatedTooltip({
  rootRef,
  delay = 250,
}: {
  rootRef: React.RefObject<HTMLElement | null>;
  delay?: number;
}) {
  const tooltipId = React.useId();
  const tooltipRef = React.useRef<HTMLDivElement>(null);
  const targetRef = React.useRef<HTMLElement | null>(null);
  const timerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);
  const [mounted, setMounted] = React.useState(false);
  const [content, setContent] = React.useState("");
  const [position, setPosition] = React.useState<{
    left: number;
    top: number;
  } | null>(null);

  const clearTimer = React.useCallback(() => {
    if (!timerRef.current) return;
    clearTimeout(timerRef.current);
    timerRef.current = null;
  }, []);

  const hide = React.useCallback(() => {
    clearTimer();
    const target = targetRef.current;
    if (target?.getAttribute("aria-describedby") === tooltipId) {
      target.removeAttribute("aria-describedby");
    }
    targetRef.current = null;
    setContent("");
    setPosition(null);
  }, [clearTimer, tooltipId]);

  const show = React.useCallback(
    (target: HTMLElement, immediately: boolean) => {
      clearTimer();
      targetRef.current = target;
      const text = target.dataset.tooltip?.trim();
      if (!text) return;

      const open = () => {
        if (targetRef.current !== target) return;
        target.setAttribute("aria-describedby", tooltipId);
        setContent(text);
      };
      if (immediately) open();
      else timerRef.current = setTimeout(open, delay);
    },
    [clearTimer, delay, tooltipId],
  );

  const updatePosition = React.useCallback(() => {
    const target = targetRef.current;
    const tooltip = tooltipRef.current;
    if (!target || !tooltip) return;

    const targetRect = target.getBoundingClientRect();
    const tooltipRect = tooltip.getBoundingClientRect();
    const viewportPadding = 8;
    const gap = 8;
    const left = Math.min(
      window.innerWidth - tooltipRect.width - viewportPadding,
      Math.max(
        viewportPadding,
        targetRect.left + targetRect.width / 2 - tooltipRect.width / 2,
      ),
    );
    const top =
      targetRect.top - tooltipRect.height - gap >= viewportPadding
        ? targetRect.top - tooltipRect.height - gap
        : targetRect.bottom + gap;
    setPosition({ left, top });
  }, []);

  React.useEffect(() => {
    setMounted(true);
    return hide;
  }, [hide]);

  React.useEffect(() => {
    const root = rootRef.current;
    if (!root) return;

    const tooltipTarget = (eventTarget: EventTarget | null) => {
      if (!(eventTarget instanceof HTMLElement)) return null;
      const target = eventTarget.closest<HTMLElement>("[data-tooltip]");
      return target && root.contains(target) ? target : null;
    };
    const onMouseOver = (event: MouseEvent) => {
      const target = tooltipTarget(event.target);
      if (!target || target === targetRef.current) return;
      show(target, false);
    };
    const onMouseOut = (event: MouseEvent) => {
      const target = tooltipTarget(event.target);
      if (!target || target !== targetRef.current) return;
      if (
        event.relatedTarget instanceof Node &&
        target.contains(event.relatedTarget)
      ) {
        return;
      }
      hide();
    };
    const onFocusIn = (event: FocusEvent) => {
      const target = tooltipTarget(event.target);
      if (target) show(target, true);
    };
    const onFocusOut = (event: FocusEvent) => {
      const target = tooltipTarget(event.target);
      if (!target || target !== targetRef.current) return;
      if (
        event.relatedTarget instanceof Node &&
        target.contains(event.relatedTarget)
      ) {
        return;
      }
      hide();
    };

    root.addEventListener("mouseover", onMouseOver);
    root.addEventListener("mouseout", onMouseOut);
    root.addEventListener("focusin", onFocusIn);
    root.addEventListener("focusout", onFocusOut);
    root.addEventListener("pointerdown", hide);
    return () => {
      root.removeEventListener("mouseover", onMouseOver);
      root.removeEventListener("mouseout", onMouseOut);
      root.removeEventListener("focusin", onFocusIn);
      root.removeEventListener("focusout", onFocusOut);
      root.removeEventListener("pointerdown", hide);
    };
  }, [hide, rootRef, show]);

  React.useLayoutEffect(() => {
    if (!content) return;
    updatePosition();
    const frame = requestAnimationFrame(updatePosition);
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [content, updatePosition]);

  React.useEffect(() => {
    if (!content) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") hide();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [content, hide]);

  if (!mounted || !content) return null;
  return createPortal(
    <div
      ref={tooltipRef}
      id={tooltipId}
      role="tooltip"
      style={{
        left: position?.left ?? 0,
        top: position?.top ?? 0,
        visibility: position ? "visible" : "hidden",
      }}
      className="pointer-events-none fixed z-[300] max-w-72 rounded-md bg-neutral-900 px-2.5 py-1.5 text-center text-xs leading-4 text-white shadow-lg"
    >
      {content}
    </div>,
    document.body,
  );
}

export function NativeTitleTooltipArea({
  rootRef,
}: {
  rootRef: React.RefObject<HTMLElement | null>;
}) {
  React.useEffect(() => {
    const root = rootRef.current;
    if (!root) return;

    const migrate = (element: Element) => {
      if (element instanceof HTMLElement && element.title) {
        element.dataset.tooltip = element.title;
        element.removeAttribute("title");
      }
      element.querySelectorAll<HTMLElement>("[title]").forEach((child) => {
        if (!child.title) return;
        child.dataset.tooltip = child.title;
        child.removeAttribute("title");
      });
    };
    migrate(root);

    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.type === "attributes") {
          migrate(mutation.target as Element);
          continue;
        }
        mutation.addedNodes.forEach((node) => {
          if (node instanceof Element) migrate(node);
        });
      }
    });
    observer.observe(root, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ["title"],
    });
    return () => observer.disconnect();
  }, [rootRef]);

  return <DelegatedTooltip rootRef={rootRef} />;
}
