"use client";

import * as React from "react";
import { createPortal } from "react-dom";
import { cn } from "@client/lib/utils";
import { getFocusableElements, trapFocus } from "@client/lib/focus";

interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: React.ReactNode;
  contentClassName?: string;
}

const DialogContext = React.createContext<{ titleId: string } | null>(null);

export function Dialog({
  open,
  onOpenChange,
  children,
  contentClassName,
}: DialogProps) {
  const [mounted, setMounted] = React.useState(false);
  const titleId = React.useId();
  const layerRef = React.useRef<HTMLDivElement>(null);
  const contentRef = React.useRef<HTMLDivElement>(null);
  const previousFocusRef = React.useRef<HTMLElement | null>(null);
  React.useEffect(() => setMounted(true), []);

  React.useEffect(() => {
    if (!open) {
      const rememberFocus = () => {
        const active = document.activeElement;
        if (
          active instanceof HTMLElement &&
          !active.closest("[data-dialog-layer]")
        ) {
          previousFocusRef.current = active;
        }
      };
      rememberFocus();
      document.addEventListener("focusin", rememberFocus);
      return () => document.removeEventListener("focusin", rememberFocus);
    }

    const previousFocus = previousFocusRef.current;
    const focusFrame = requestAnimationFrame(() => {
      const content = contentRef.current;
      if (!content || content.contains(document.activeElement)) return;
      (getFocusableElements(content)[0] ?? content).focus();
    });

    const isTopmost = () => {
      const layers = document.querySelectorAll("[data-dialog-layer]");
      return layers[layers.length - 1] === layerRef.current;
    };

    const onKey = (event: KeyboardEvent) => {
      const content = contentRef.current;
      if (!content || !isTopmost()) return;

      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onOpenChange(false);
        return;
      }

      trapFocus(event, content);
    };

    document.addEventListener("keydown", onKey);
    return () => {
      cancelAnimationFrame(focusFrame);
      document.removeEventListener("keydown", onKey);
      previousFocus?.focus();
      previousFocusRef.current = null;
    };
  }, [open, onOpenChange]);

  if (!mounted || !open) return null;

  return createPortal(
    <div
      ref={layerRef}
      data-dialog-layer
      className="fixed inset-0 z-[200] flex items-center justify-center"
    >
      <div
        className="absolute inset-0 bg-black/40"
        onClick={() => onOpenChange(false)}
        aria-hidden
      />
      <div
        ref={contentRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        className={cn(
          "relative z-10 w-[min(480px,90vw)] rounded-lg bg-white p-6 shadow-xl outline-none",
          contentClassName,
        )}
      >
        <DialogContext.Provider value={{ titleId }}>
          {children}
        </DialogContext.Provider>
      </div>
    </div>,
    document.body,
  );
}

export function DialogHeader({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("mb-4 flex flex-col gap-1", className)} {...props} />;
}

export function DialogTitle({
  className,
  id,
  ...props
}: React.HTMLAttributes<HTMLHeadingElement>) {
  const context = React.useContext(DialogContext);
  return (
    <h2
      id={id ?? context?.titleId}
      className={cn("text-lg font-semibold", className)}
      {...props}
    />
  );
}

export function DialogFooter({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("mt-6 flex justify-end gap-2", className)} {...props} />;
}
