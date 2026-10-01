import * as React from "react";

const MARGIN = 8;

/** Keep the measured popover in view without moving it when content shrinks. */
export function usePopoverPosition(
  ref: React.RefObject<HTMLDivElement | null>,
  anchor: { left: number; top: number },
  mounted: boolean,
) {
  const [position, setPosition] = React.useState(() => ({ ...anchor }));

  React.useLayoutEffect(() => {
    const element = ref.current;
    if (!mounted || !element) return;

    const fitInViewport = () => {
      const { width, height } = element.getBoundingClientRect();
      setPosition((current) => {
        const left = Math.max(
          MARGIN,
          Math.min(current.left, window.innerWidth - width - MARGIN),
        );
        const top = Math.max(
          MARGIN,
          Math.min(current.top, window.innerHeight - height - MARGIN),
        );
        return left === current.left && top === current.top
          ? current
          : { left, top };
      });
    };

    fitInViewport();
    const observer = new ResizeObserver(fitInViewport);
    observer.observe(element);
    window.addEventListener("resize", fitInViewport);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", fitInViewport);
    };
  }, [mounted, ref]);

  return {
    ...position,
    maxWidth: `calc(100vw - ${MARGIN * 2}px)`,
    maxHeight: `calc(100dvh - ${MARGIN * 2}px)`,
  };
}
