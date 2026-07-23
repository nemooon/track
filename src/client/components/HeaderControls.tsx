import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cn } from "@client/lib/utils";

export const headerControlGroupClass =
  "inline-flex shrink-0 items-center gap-0.5 rounded-lg bg-white/10 p-0.5";

export function headerControlItemClass(
  active = false,
  iconOnly = false,
  className?: string,
) {
  return cn(
    "inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-md text-xs font-medium transition-[background-color,color,box-shadow] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/60 disabled:pointer-events-none disabled:opacity-40",
    iconOnly ? "w-8" : "px-2.5",
    active
      ? "bg-[#f3f5f4] text-[#26332e] shadow-sm shadow-black/20"
      : "text-white/65 hover:bg-white/10 hover:text-white",
    className,
  );
}

export function HeaderControlGroup({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn(headerControlGroupClass, className)}>{children}</div>
  );
}

export interface HeaderControlButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement> {
  active?: boolean;
  iconOnly?: boolean;
}

export function HeaderControlButton({
  active = false,
  iconOnly = false,
  className,
  type = "button",
  ...props
}: HeaderControlButtonProps) {
  return (
    <button
      type={type}
      className={headerControlItemClass(active, iconOnly, className)}
      {...props}
    />
  );
}
