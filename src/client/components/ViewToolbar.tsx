import type { ReactNode } from "react";
import { cn } from "@client/lib/utils";

export function ViewToolbar({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      role="toolbar"
      aria-label="ビュー操作"
      className={cn(
        "flex min-h-12 shrink-0 flex-wrap items-center gap-2 border-b border-neutral-200 bg-white px-3 py-2",
        className,
      )}
    >
      {children}
    </div>
  );
}
