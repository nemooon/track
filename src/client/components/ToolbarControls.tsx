import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cn } from "@client/lib/utils";

export const toolbarControlGroupClass =
  "inline-flex shrink-0 items-center gap-0.5 rounded-lg bg-neutral-100 p-0.5";

export function toolbarControlItemClass(
  active = false,
  iconOnly = false,
  className?: string,
) {
  return cn(
    "inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-md text-xs font-medium transition-[background-color,color,box-shadow] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-neutral-400 disabled:pointer-events-none disabled:opacity-40",
    iconOnly ? "w-8" : "px-2.5",
    active
      ? "bg-white text-neutral-900 shadow-sm ring-1 ring-black/5"
      : "text-neutral-500 hover:bg-white/70 hover:text-neutral-900",
    className,
  );
}

export function ToolbarControlGroup({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn(toolbarControlGroupClass, className)}>{children}</div>
  );
}

export interface ToolbarControlButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement> {
  active?: boolean;
  iconOnly?: boolean;
}

export function ToolbarControlButton({
  active = false,
  iconOnly = false,
  className,
  type = "button",
  ...props
}: ToolbarControlButtonProps) {
  return (
    <button
      type={type}
      className={toolbarControlItemClass(active, iconOnly, className)}
      {...props}
    />
  );
}
