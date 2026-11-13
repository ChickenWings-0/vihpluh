import { useEffect, useState } from "react";

/** The window's CSS width, updated on resize. */
export function useWindowWidth(): number {
  const [width, setWidth] = useState(() => window.innerWidth);
  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return width;
}

/**
 * Responsive collapse steps by window width (DESIGN_SYSTEM §6.4):
 * 0 = everything; 1 = < 900; 2 = < 720; 3 = < 560.
 */
export function collapseLevel(width: number): 0 | 1 | 2 | 3 {
  if (width < 560) return 3;
  if (width < 720) return 2;
  if (width < 900) return 1;
  return 0;
}
