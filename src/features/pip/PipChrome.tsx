/**
 * Picture-in-picture chrome (DESIGN_SYSTEM §9): nothing but the picture until the pointer is
 * over the window; then a 28 px strip with Play/Pause and a seek line along the bottom, and
 * Restore and Close at the top right. The rest of the surface drags the window (the
 * interaction layer does that), and the edges resize it keeping the picture's shape.
 */
import { Maximize2, Pause, Play, X } from "lucide-react";
import { type PointerEvent, useEffect, useRef, useState } from "react";
import { dispatch } from "@/actions";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { Tooltip } from "@/components/Tooltip";
import { playerSeek, windowClose, windowPipRefit, windowPipResize } from "@/ipc/commands";
import { type ResizeEdge, startWindowResize } from "@/ipc/transport";
import { useAnimationFrame } from "@/lib/useAnimationFrame";
import { useAppStore } from "@/stores/app";
import { useShortcut } from "@/stores/keymap";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { getDuration, getPosition } from "@/stores/time";

const ZONES: { edge: ResizeEdge; className: string }[] = [
  { edge: "North", className: "top-0 inset-x-3 h-1.5 cursor-ns-resize" },
  { edge: "South", className: "bottom-0 inset-x-3 h-1.5 cursor-ns-resize" },
  { edge: "West", className: "left-0 inset-y-3 w-1.5 cursor-ew-resize" },
  { edge: "East", className: "right-0 inset-y-3 w-1.5 cursor-ew-resize" },
  { edge: "NorthWest", className: "top-0 left-0 h-3 w-3 cursor-nwse-resize" },
  { edge: "NorthEast", className: "top-0 right-0 h-3 w-3 cursor-nesw-resize" },
  { edge: "SouthWest", className: "bottom-0 left-0 h-3 w-3 cursor-nesw-resize" },
  { edge: "SouthEast", className: "bottom-0 right-0 h-3 w-3 cursor-nwse-resize" },
];

const RESIZE_INTERVAL_MS = 16;

/**
 * Edge and corner zones. Where the session can place windows, a drag is sent to the core,
 * which keeps the picture's shape; elsewhere (Wayland) the compositor's own resize is used.
 */
function PipResizeZones() {
  const canPosition = useAppStore((s) => s.info?.capabilities.windowPositioning ?? true);
  const drag = useRef<{ edge: ResizeEdge; x: number; y: number; sent: number } | null>(null);

  const onDown = (edge: ResizeEdge) => (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    if (!canPosition) {
      void startWindowResize(edge).catch(reportError);
      return;
    }
    e.currentTarget.setPointerCapture(e.pointerId);
    // Screen coordinates: the window moves under the pointer while it is resized.
    drag.current = { edge, x: e.screenX, y: e.screenY, sent: 0 };
    void windowPipResize("start", edge, 0, 0).catch(reportError);
  };
  const onMove = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    const now = performance.now();
    if (now - d.sent < RESIZE_INTERVAL_MS) return;
    d.sent = now;
    void windowPipResize("move", d.edge, e.screenX - d.x, e.screenY - d.y).catch(reportError);
  };
  const onUp = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    void windowPipResize("move", d.edge, e.screenX - d.x, e.screenY - d.y)
      .then(() => windowPipResize("end", d.edge, 0, 0))
      .catch(reportError);
  };

  return (
    <>
      {ZONES.map(({ edge, className }) => (
        <div
          key={edge}
          aria-hidden="true"
          data-pip-resize={edge}
          onPointerDown={onDown(edge)}
          onPointerMove={onMove}
          onPointerUp={onUp}
          onPointerCancel={onUp}
          className={`absolute z-(--z-tooltip) touch-none ${className}`}
        />
      ))}
    </>
  );
}

/** The seek line: 4 px, 8 px while the pointer is on it. Click or drag to seek. */
function SeekLine() {
  const fill = useRef<HTMLDivElement>(null);
  const track = useRef<HTMLDivElement>(null);
  const hasMedia = usePlayer((s) => s.media !== null) ?? false;
  useAnimationFrame(() => {
    if (!fill.current) return;
    const duration = getDuration();
    const done = duration > 0 ? Math.min(1, Math.max(0, getPosition() / duration)) : 0;
    const width = `${(done * 100).toFixed(2)}%`;
    if (fill.current.style.width !== width) fill.current.style.width = width;
  });
  const seekTo = (clientX: number) => {
    const rect = track.current?.getBoundingClientRect();
    const duration = getDuration();
    if (!rect || rect.width === 0 || duration <= 0) return;
    const at = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width)) * duration;
    void playerSeek({ kind: "absolute", value: at, exact: false }).catch(reportError);
  };
  return (
    <div
      ref={track}
      role="slider"
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={0}
      tabIndex={-1}
      onPointerDown={(e) => {
        if (e.button !== 0 || !hasMedia) return;
        e.stopPropagation();
        e.currentTarget.setPointerCapture(e.pointerId);
        seekTo(e.clientX);
      }}
      onPointerMove={(e) => {
        if (e.currentTarget.hasPointerCapture(e.pointerId)) seekTo(e.clientX);
      }}
      onPointerUp={(e) => {
        if (e.currentTarget.hasPointerCapture(e.pointerId)) {
          e.currentTarget.releasePointerCapture(e.pointerId);
        }
      }}
      className="group/seek flex h-3 min-w-0 flex-1 cursor-pointer touch-none items-center"
    >
      <div className="h-1 w-full overflow-hidden rounded-track bg-track transition-[height] duration-(--dur-fast) group-hover/seek:h-2">
        <div ref={fill} className="h-full bg-accent" />
      </div>
    </div>
  );
}

const BUTTON =
  "flex h-6 w-6 shrink-0 items-center justify-center rounded-control bg-surface/92 text-fg-2 hover:bg-hover hover:text-fg";

export function PipChrome() {
  const playing = usePlayer((s) => s.status === "playing") ?? false;
  const mediaKey = usePlayer((s) => s.media?.key ?? "") ?? "";
  const videoTrack = usePlayer((s) => s.selected.video) ?? null;
  const pauseKey = useShortcut("player.toggle_pause");
  const pipKey = useShortcut("window.toggle_pip");
  const [hovered, setHovered] = useState(false);

  // Another file (or track) may have another shape: keep the window fitted to the picture.
  // biome-ignore lint/correctness/useExhaustiveDependencies: refits when what is playing changes
  useEffect(() => {
    const timer = setTimeout(() => void windowPipRefit().catch(() => undefined), 400);
    return () => clearTimeout(timer);
  }, [mediaKey, videoTrack]);

  useEffect(() => {
    const root = document.documentElement;
    const enter = () => setHovered(true);
    const leave = () => setHovered(false);
    root.addEventListener("pointerenter", enter);
    root.addEventListener("pointermove", enter, { passive: true });
    root.addEventListener("pointerleave", leave);
    return () => {
      root.removeEventListener("pointerenter", enter);
      root.removeEventListener("pointermove", enter);
      root.removeEventListener("pointerleave", leave);
    };
  }, []);

  const shown = hovered ? "opacity-100" : "pointer-events-none opacity-0";
  const click = (run: () => void) => (e: { detail: number }) => {
    run();
    if (e.detail > 0) focusPlayerRoot();
  };

  return (
    <>
      <div
        data-pip-chrome
        className={`absolute top-1.5 right-1.5 z-(--z-chrome) flex gap-1 transition-opacity duration-(--dur-fast) ${shown}`}
      >
        <Tooltip label="Restore" shortcut={pipKey} side="bottom">
          <button
            type="button"
            aria-label="Restore the window"
            onClick={click(() => void dispatch("window.toggle_pip", { trigger: "button" }))}
            className={BUTTON}
          >
            <Maximize2 size={14} strokeWidth={1.5} aria-hidden />
          </button>
        </Tooltip>
        <Tooltip label="Close" side="bottom">
          <button
            type="button"
            aria-label="Close"
            onClick={() => void windowClose().catch(reportError)}
            className={`${BUTTON} hover:bg-danger`}
          >
            <X size={14} strokeWidth={1.5} aria-hidden />
          </button>
        </Tooltip>
      </div>
      <div
        role="toolbar"
        aria-label="Playback"
        className={`absolute inset-x-0 bottom-0 z-(--z-chrome) flex h-7 items-center gap-2 bg-surface/92 pr-3 pl-1 transition-opacity duration-(--dur-fast) ${shown}`}
      >
        <Tooltip label={playing ? "Pause" : "Play"} shortcut={pauseKey}>
          <button
            type="button"
            aria-label={playing ? "Pause" : "Play"}
            onClick={click(() => void dispatch("player.toggle_pause", { trigger: "button" }))}
            className="flex h-6 w-6 shrink-0 items-center justify-center rounded-control text-fg hover:bg-hover"
          >
            {playing ? (
              <Pause size={14} strokeWidth={1.5} fill="currentColor" aria-hidden />
            ) : (
              <Play size={14} strokeWidth={1.5} fill="currentColor" aria-hidden />
            )}
          </button>
        </Tooltip>
        <SeekLine />
      </div>
      <PipResizeZones />
    </>
  );
}
