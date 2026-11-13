/**
 * The right dock (DESIGN_SYSTEM §6.1, §6.3): one panel at a time, between the bars.
 *
 * - Dock mode shrinks the picture through mpv's video margins (D5), animated over 160 ms, so
 *   the panel never covers the video. Overlay mode floats the panel at --chrome-alpha.
 * - `interface.sidebar_mode` "auto" docks in a window and overlays in fullscreen; below 560 px
 *   panels always overlay (§6.4). Zen hides the dock and gives the picture back.
 * - The left edge drags to resize (320–480 px), saved to `interface.sidebar_width`.
 */
import { type PointerEvent, useEffect, useRef } from "react";
import { configSet, videoSetMargins } from "@/ipc/commands";
import { useWindowWidth } from "@/lib/useWindowWidth";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";
import { QueuePanel } from "../queue/QueuePanel";
import { Sidebar } from "../sidebar/Sidebar";

const MIN_WIDTH = 320;
const MAX_WIDTH = 480;
const ANIMATION_MS = 160;

let currentRight = 0;
let animation = 0;

/** Animates the right video margin to `target` (a fraction of the window width). */
function animateMargin(target: number, reduceMotion: boolean) {
  cancelAnimationFrame(animation);
  const from = currentRight;
  if (Math.abs(from - target) < 0.0005) return;
  if (reduceMotion) {
    currentRight = target;
    void videoSetMargins({ left: 0, right: target, top: 0, bottom: 0 }).catch(reportError);
    return;
  }
  const start = performance.now();
  const step = (now: number) => {
    const t = Math.min(1, (now - start) / ANIMATION_MS);
    // --ease-out: cubic-bezier(0.2, 0, 0, 1) ≈ easeOutCubic for this purpose.
    const eased = 1 - (1 - t) ** 3;
    currentRight = from + (target - from) * eased;
    void videoSetMargins({ left: 0, right: currentRight, top: 0, bottom: 0 }).catch(reportError);
    if (t < 1) animation = requestAnimationFrame(step);
  };
  animation = requestAnimationFrame(step);
}

export function useDockLayout() {
  const dock = useUiStore((s) => s.dock);
  const zen = useUiStore((s) => s.zen);
  const fullscreen = useUiStore((s) => s.window.mode === "fullscreen");
  const dragWidth = useUiStore((s) => s.dockDragWidth);
  const mode = useConfig((c) => c.interface.sidebar_mode) ?? "auto";
  const configured = useConfig((c) => c.interface.sidebar_width) ?? 360;
  const windowWidth = useWindowWidth();
  const width = Math.min(dragWidth ?? configured, Math.max(0, windowWidth - 160));
  const overlay = mode === "overlay" || (mode === "auto" && fullscreen) || windowWidth < 560;
  return { dock: zen ? null : dock, width, overlay, windowWidth };
}

export function Dock() {
  const { dock, width, overlay, windowWidth } = useDockLayout();
  const reduceMotion = useConfig((c) => c.interface.reduce_motion) === "on";
  const setDockDragWidth = useUiStore((s) => s.setDockDragWidth);
  const setKeyContext = useUiStore((s) => s.setKeyContext);
  const dragging = useRef(false);

  // The picture moves aside only in dock mode.
  useEffect(() => {
    const target = dock && !overlay && windowWidth > 0 ? width / windowWidth : 0;
    animateMargin(target, reduceMotion || dragging.current);
  }, [dock, overlay, width, windowWidth, reduceMotion]);

  if (!dock) return null;

  const onResizeDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    dragging.current = true;
  };
  const onResizeMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!dragging.current) return;
    const next = Math.round(
      Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, window.innerWidth - e.clientX)),
    );
    setDockDragWidth(next);
  };
  const onResizeUp = (e: PointerEvent<HTMLDivElement>) => {
    if (!dragging.current) return;
    dragging.current = false;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    const final = Math.round(
      Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, window.innerWidth - e.clientX)),
    );
    void configSet("interface.sidebar_width", final)
      .catch(reportError)
      .finally(() => setDockDragWidth(null));
  };

  return (
    <aside
      aria-label={dock === "queue" ? "Queue" : "Sidebar"}
      onFocus={() => setKeyContext(dock === "queue" ? "queue" : "global")}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setKeyContext("global");
      }}
      className={`absolute top-10 right-0 bottom-13 z-(--z-dock) flex flex-col border-line border-l ${
        overlay ? "bg-surface/92 shadow-float" : "bg-surface"
      }`}
      style={{ width }}
    >
      <div
        aria-hidden="true"
        onPointerDown={onResizeDown}
        onPointerMove={onResizeMove}
        onPointerUp={onResizeUp}
        className="absolute inset-y-0 -left-1 w-2 cursor-ew-resize"
      />
      {dock === "queue" ? <QueuePanel /> : <Sidebar width={width} />}
    </aside>
  );
}
