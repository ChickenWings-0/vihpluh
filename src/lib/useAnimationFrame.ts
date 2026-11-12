import { useEffect, useRef } from "react";

/**
 * Calls `callback` once per animation frame while mounted. For high-frequency values
 * (position, drag state) that must update the DOM without re-rendering React.
 */
export function useAnimationFrame(callback: (now: number) => void): void {
  const latest = useRef(callback);
  latest.current = callback;
  useEffect(() => {
    let frame = 0;
    const tick = (now: number) => {
      latest.current(now);
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, []);
}
