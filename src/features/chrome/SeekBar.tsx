/**
 * Seek bar v1 (DESIGN_SYSTEM §8.1): thick, forgiving, squared-off. Layers bottom→top: track,
 * buffered range, played range, chapter gaps, hover ghost, thumb. Position, buffer, and hover
 * are written straight to the DOM every animation frame — never through React state.
 *
 * Click = exact seek. Drag = keyframe seeks throttled to 50 ms, then one exact seek on release.
 * Wheel = ± small step. With keyboard focus: ←/→ small step, Shift+←/→ exactly 1 s, Home/End.
 * Thumbnails and the waveform mode arrive in Phase 6.
 */
import { type KeyboardEvent, type PointerEvent, useRef, useState, type WheelEvent } from "react";
import { playerSeek } from "@/ipc/commands";
import { formatTime } from "@/lib/time";
import { useAnimationFrame } from "@/lib/useAnimationFrame";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { getCacheEnd, getDuration, getPosition } from "@/stores/time";
import { useUiStore } from "@/stores/ui";

const DRAG_SEEK_MS = 50;

export function SeekBar() {
  const root = useRef<HTMLDivElement>(null);
  const played = useRef<HTMLDivElement>(null);
  const buffered = useRef<HTMLDivElement>(null);
  const ghost = useRef<HTMLDivElement>(null);
  const ghostLine = useRef<HTMLDivElement>(null);
  const thumb = useRef<HTMLDivElement>(null);
  const label = useRef<HTMLDivElement>(null);
  const hoverX = useRef<number | null>(null);
  const dragTo = useRef<number | null>(null);
  const lastDragSeek = useRef(0);
  const [active, setActive] = useState(false);
  const hold = useUiStore((s) => s.hold);

  const hasMedia = usePlayer((s) => s.media !== null) ?? false;
  const chapters = usePlayer((s) => s.chapters) ?? [];
  const showChapters = useConfig((c) => c.interface.seek_chapter_marks) ?? true;
  const precise = useConfig((c) => c.controls.precise_seek) ?? true;
  const smallStep = useConfig((c) => c.controls.seek_small_s) ?? 5;

  const timeAt = (clientX: number) => {
    const rect = root.current?.getBoundingClientRect();
    const dur = getDuration();
    if (!rect || rect.width === 0 || dur <= 0) return 0;
    return Math.min(dur, Math.max(0, ((clientX - rect.left) / rect.width) * dur));
  };

  useAnimationFrame(() => {
    const dur = getDuration();
    const pos = dragTo.current ?? getPosition();
    const frac = dur > 0 ? Math.min(1, pos / dur) : 0;
    if (played.current) played.current.style.transform = `scaleX(${frac})`;
    if (thumb.current) thumb.current.style.left = `${frac * 100}%`;
    if (buffered.current) {
      // Network/cache only matters ahead of the playhead.
      const end = dur > 0 ? Math.min(1, getCacheEnd() / dur) : 0;
      buffered.current.style.transform = `scaleX(${Math.max(end, frac)})`;
    }
    const x = hoverX.current;
    const rect = root.current?.getBoundingClientRect();
    if (ghost.current && ghostLine.current && label.current && rect) {
      if (x === null || dur <= 0) {
        ghost.current.style.opacity = "0";
        ghostLine.current.style.opacity = "0";
        label.current.style.opacity = "0";
        // Park it inside the bar, so a stale position never widens the layout.
        label.current.style.left = "0px";
      } else {
        const hoverFrac = Math.min(1, Math.max(0, (x - rect.left) / rect.width));
        const [a, b] = hoverFrac >= frac ? [frac, hoverFrac] : [hoverFrac, frac];
        ghost.current.style.opacity = "1";
        ghost.current.style.left = `${a * 100}%`;
        ghost.current.style.width = `${(b - a) * 100}%`;
        ghostLine.current.style.opacity = "1";
        ghostLine.current.style.left = `${hoverFrac * 100}%`;
        const t = hoverFrac * dur;
        const chapter = [...chapters].reverse().find((c) => c.time <= t);
        label.current.textContent = chapter ? `${chapter.title} · ${formatTime(t)}` : formatTime(t);
        label.current.style.opacity = "1";
        // Keep the label inside the window (edge clamping, no layout shift).
        const half = label.current.offsetWidth / 2;
        const px = Math.min(window.innerWidth - 8 - half, Math.max(8 + half, x));
        label.current.style.left = `${px - rect.left}px`;
      }
    }
  });

  const seek = (value: number, exact: boolean) =>
    playerSeek({ kind: "absolute", value, exact }).catch(reportError);

  // The drag is tracked explicitly: pointer capture can be lost mid-drag (focus changes, OS
  // interruptions), and the release must still commit the seek.
  const dragging = useRef(false);

  const finishDrag = (target: number) => {
    if (!dragging.current) return;
    dragging.current = false;
    hold("drag", false);
    void seek(target, precise).finally(() => {
      dragTo.current = null;
    });
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || !hasMedia) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    dragging.current = true;
    hold("drag", true);
    dragTo.current = timeAt(e.clientX);
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    hoverX.current = e.clientX;
    if (!dragging.current) return;
    dragTo.current = timeAt(e.clientX);
    const now = performance.now();
    if (now - lastDragSeek.current >= DRAG_SEEK_MS) {
      lastDragSeek.current = now;
      void seek(dragTo.current, false);
    }
  };
  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    finishDrag(timeAt(e.clientX));
  };
  const onLostPointerCapture = () => {
    if (dragTo.current !== null) finishDrag(dragTo.current);
  };

  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    if (!hasMedia || Math.abs(e.deltaY) < 1) return;
    e.stopPropagation();
    const delta = e.deltaY < 0 ? smallStep : -smallStep;
    void playerSeek({ kind: "relative", value: delta, exact: precise }).catch(reportError);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const dur = getDuration();
    let req: Parameters<typeof playerSeek>[0] | null = null;
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      const sign = e.key === "ArrowLeft" ? -1 : 1;
      req = e.shiftKey
        ? { kind: "relative", value: sign, exact: true }
        : { kind: "relative", value: sign * smallStep, exact: precise };
    } else if (e.key === "Home") req = { kind: "absolute", value: 0, exact: true };
    else if (e.key === "End" && dur > 0) req = { kind: "absolute", value: dur - 1, exact: true };
    if (!req) return;
    e.preventDefault();
    e.stopPropagation();
    void playerSeek(req).catch(reportError);
  };

  const dur = getDuration();
  const thick = active ? "h-3" : "h-2";

  return (
    <div
      ref={root}
      role="slider"
      tabIndex={hasMedia ? 0 : -1}
      aria-label="Position"
      aria-valuemin={0}
      aria-valuemax={Math.round(dur)}
      aria-valuenow={Math.round(getPosition())}
      aria-valuetext={`${formatTime(getPosition())} of ${formatTime(dur)}`}
      aria-disabled={!hasMedia}
      onPointerEnter={() => setActive(true)}
      onPointerLeave={() => {
        hoverX.current = null;
        if (!dragTo.current) setActive(false);
      }}
      onPointerDown={onPointerDown}
      // Pointer use never moves focus to a control (DESIGN_SYSTEM §13); Tab still does.
      onMouseDown={(e) => e.preventDefault()}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onLostPointerCapture={onLostPointerCapture}
      onWheel={onWheel}
      onKeyDown={onKeyDown}
      onFocus={(e) => setActive(e.currentTarget.matches(":focus-visible"))}
      onBlur={() => setActive(false)}
      className={`group relative flex h-7 min-w-40 flex-1 touch-none items-center rounded-control ${
        hasMedia ? "cursor-pointer" : ""
      }`}
    >
      <div
        className={`relative w-full overflow-hidden rounded-control bg-track transition-[height] duration-(--dur-fast) ease-out ${thick}`}
      >
        <div ref={buffered} className="absolute inset-0 origin-left bg-track-buffer" />
        <div ref={played} className="absolute inset-0 origin-left bg-accent" />
        <div ref={ghost} className="absolute inset-y-0 bg-ghost opacity-0" />
        {showChapters &&
          dur > 0 &&
          chapters
            .filter((c) => c.time > 0.5)
            .map((c) => (
              <div
                key={c.index}
                className="absolute inset-y-0 w-0.5 -translate-x-1/2 bg-surface"
                style={{ left: `${(c.time / dur) * 100}%` }}
              />
            ))}
      </div>
      <div
        ref={ghostLine}
        className="pointer-events-none absolute top-1/2 h-3 w-px -translate-y-1/2 bg-fg opacity-0"
      />
      <div
        ref={thumb}
        className={`pointer-events-none absolute top-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-fg transition-opacity duration-(--dur-fast) ${
          active && hasMedia ? "opacity-100" : "opacity-0"
        }`}
      />
      <div
        ref={label}
        className="tabular pointer-events-none absolute bottom-full mb-3 -translate-x-1/2 whitespace-nowrap rounded-control border border-line-strong bg-raised px-2 py-0.5 text-fg text-sm opacity-0 shadow-float"
      />
    </div>
  );
}
