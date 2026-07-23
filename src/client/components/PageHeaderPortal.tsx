import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

export type PageHeaderSlot = "center" | "right";

export function PageHeaderPortal({
  slot,
  children,
}: {
  slot: PageHeaderSlot;
  children: ReactNode;
}) {
  const [target, setTarget] = useState<HTMLElement | null>(null);

  useEffect(() => {
    setTarget(document.getElementById(`page-header-${slot}`));
  }, [slot]);

  return target ? createPortal(children, target) : null;
}
