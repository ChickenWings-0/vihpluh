/**
 * The seek bar (DESIGN_SYSTEM §8.1): thick, forgiving, squared-off. Layers bottom→top: track,
 * buffered range, played range, chapter gaps, hover ghost, thumb. Position, buffer, hover, and
 * the preview card are written straight to the DOM every animation frame — never through
 * React state.
 *
 * Click = exact seek. Drag = keyframe seeks throttled to 50 ms, then one exact seek on release.
 * Wheel = ± small step. With keyboard focus: ←/→ small step, Shift+←/→ exactly 1 s, Home/End.
 *
 * Hover preview: a card above the pointer with the thumbnail (`vthumb`, §7.5), chapter, and
 * time. The thumbnail swaps in only once loaded — no spinner, no layout shift — and is left
 * out for audio and streams. Waveform mode (`interface.seek_waveform`) draws the track as the
 * audio waveform: two canvases (played/unplayed colours) drawn once, the played part revealed
 * by a clip each frame.
 */
import {
  type KeyboardEvent,
  type PointerEvent,
  type RefObject,
  useEffect,
  useRef,
  useState,
  type WheelEvent,
} from "react";
import { playerSeek } from "@/ipc/commands";
import { thumbUrl } from "@/ipc/transport";
import { formatTime } from "@/lib/time";
import { useAnimationFrame } from "@/lib/useAnimationFrame";
import { binPeaks, loudest, type Waveform } from "@/lib/waveform";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { getCacheEnd, getDuration, getPosition } from "@/stores/time";
import { useUiStore } from "@/stores/ui";
import { useWaveform } from "./useWaveform";

const DRAG_SEEK_MS = 50;

/** Seconds per thumbnail bucket; must match `media::thumbs::bucket_step`. */
export function bucketStep(duration: number): number {
  if (!Number.isFinite(duration) || duration <= 0) return 2;
  return Math.min(30, Math.max(2, Math.round(duration / 300)));
}

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Two stacked canvases: unplayed waveform, and the played one revealed up to the playhead. */
function WaveformTrack({
  wave,
  duration,
  playedClip,
}: {
  wave: Waveform;
  duration: number;
  playedClip: RefObject<HTMLDivElement | null>;
}) {
  const box = useRef<HTMLDivElement>(null);
  const base = useRef<HTMLCanvasElement>(null);
  const played = useRef<HTMLCanvasElement>(null);
  const [width, setWidth] = useState(0);
  const theme = useConfig((c) => c.interface.theme);
  const accent = useConfig((c) => c.interface.accent);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setWidth(Math.round(entry.contentRect.width));
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Redrawn when the data, size, theme, or accent changes — not per frame.
  // biome-ignore lint/correctness/useExhaustiveDependencies: theme and accent change the CSS colours read inside
  useEffect(() => {
    if (width <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    const columns = Math.max(1, Math.round(width * dpr));
    const height = Math.round(28 * dpr);
    const bins = binPeaks(wave, columns, duration);
    // Scaled to the file's loudest moment so quiet mixes still read; the floor keeps a silent
    // file from turning its noise into full-height bars.
    const scale = 1 / Math.max(0.25, loudest(wave));
    const draw = (canvas: HTMLCanvasElement | null, color: string) => {
      if (!canvas) return;
      canvas.width = columns;
      canvas.height = height;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.clearRect(0, 0, columns, height);
      ctx.fillStyle = color;
      const mid = height / 2;
      for (let c = 0; c < columns; c++) {
        const lo = Math.max(-1, (bins[c * 2] ?? 0) * scale);
        const hi = Math.min(1, (bins[c * 2 + 1] ?? 0) * scale);
        const top = mid - hi * mid;
        const bottom = mid - lo * mid;
        ctx.fillRect(c, top, 1, Math.max(1, bottom - top));
      }
    };
    // Next frame: theme and accent tokens are rewritten by their own effect first.
    const frame = requestAnimationFrame(() => {
      draw(base.current, cssVar("--track-buffer"));
      draw(played.current, cssVar("--accent"));
    });
    return () => cancelAnimationFrame(frame);
  }, [wave, duration, width, theme, accent]);

  return (
    <div ref={box} className="absolute inset-0">
      <canvas ref={base} className="absolute inset-0 h-full w-full" />
      <div ref={playedClip} className="absolute inset-0">
        <canvas ref={played} className="absolute inset-0 h-full w-full" />
      </div>
    </div>
  );
}

export function SeekBar() {
  const root = useRef<HTMLDivElement>(null);
  const played = useRef<HTMLDivElement>(null);
  const playedClip = useRef<HTMLDivElement>(null);
  const buffered = useRef<HTMLDivElement>(null);
  const ghost = useRef<HTMLDivElement>(null);
  const ghostLine = useRef<HTMLDivElement>(null);
  const thumb = useRef<HTMLDivElement>(null);
  const card = useRef<HTMLDivElement>(null);
  const picture = useRef<HTMLDivElement>(null);
  const image = useRef<HTMLImageElement>(null);
  const chapterLine = useRef<HTMLDivElement>(null);
  const timeLine = useRef<HTMLDivElement>(null);
  const hoverX = useRef<number | null>(null);
  const dragTo = useRef<number | null>(null);
  const lastDragSeek = useRef(0);
  const [active, setActive] = useState(false);
  const hold = useUiStore((s) => s.hold);

  const hasMedia = usePlayer((s) => s.media !== null) ?? false;
  const mediaKey = usePlayer((s) => s.media?.key ?? "") ?? "";
  const previewable =
    usePlayer((s) => s.media !== null && s.media.kind === "video" && !s.media.isUrl) ?? false;
  const chapters = usePlayer((s) => s.chapters) ?? [];
  const showChapters = useConfig((c) => c.interface.seek_chapter_marks) ?? true;
  const seekPreview = useConfig((c) => c.interface.seek_preview) ?? true;
  const waveformOn = useConfig((c) => c.interface.seek_waveform) ?? false;
  const precise = useConfig((c) => c.controls.precise_seek) ?? true;
  const smallStep = useConfig((c) => c.controls.seek_small_s) ?? 5;
  const { wave, progress } = useWaveform();
  const waveMode = waveformOn && wave !== null;

  // Thumbnail loading: a detached loader fetches; the visible image swaps only on success.
  const wantedUrl = useRef<string | null>(null);
  const loader = useRef<HTMLImageElement | null>(null);
  const thumbsOn = seekPreview && previewable && mediaKey !== "";
  const thumbsOnRef = useRef(thumbsOn);
  thumbsOnRef.current = thumbsOn;
  const keyRef = useRef(mediaKey);
  keyRef.current = mediaKey;

  useEffect(() => {
    const img = new Image();
    img.onload = () => {
      if (img.src !== wantedUrl.current || !image.current || !picture.current) return;
      image.current.src = img.src;
      picture.current.style.display = "block";
    };
    loader.current = img;
    return () => {
      img.onload = null;
    };
  }, []);

  // A new file (or previews off) never shows the last file's picture.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset whenever the media or switch changes
  useEffect(() => {
    wantedUrl.current = null;
    if (picture.current) picture.current.style.display = "none";
  }, [mediaKey, thumbsOn]);

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
    if (playedClip.current) playedClip.current.style.clipPath = `inset(0 ${(1 - frac) * 100}% 0 0)`;
    if (thumb.current) thumb.current.style.left = `${frac * 100}%`;
    if (buffered.current) {
      // Network/cache only matters ahead of the playhead.
      const end = dur > 0 ? Math.min(1, getCacheEnd() / dur) : 0;
      buffered.current.style.transform = `scaleX(${Math.max(end, frac)})`;
    }
    const x = hoverX.current;
    const rect = root.current?.getBoundingClientRect();
    if (!ghost.current || !ghostLine.current || !card.current || !rect) return;
    if (x === null || dur <= 0) {
      ghost.current.style.opacity = "0";
      ghostLine.current.style.opacity = "0";
      card.current.style.opacity = "0";
      // Park it inside the bar, so a stale position never widens the layout.
      card.current.style.left = "0px";
      return;
    }
    const hoverFrac = Math.min(1, Math.max(0, (x - rect.left) / rect.width));
    const [a, b] = hoverFrac >= frac ? [frac, hoverFrac] : [hoverFrac, frac];
    ghost.current.style.opacity = "1";
    ghost.current.style.left = `${a * 100}%`;
    ghost.current.style.width = `${(b - a) * 100}%`;
    ghostLine.current.style.opacity = "1";
    ghostLine.current.style.left = `${hoverFrac * 100}%`;
    const t = hoverFrac * dur;
    const chapter = [...chapters].reverse().find((c) => c.time <= t);
    if (chapterLine.current) {
      chapterLine.current.textContent = chapter?.title ?? "";
      chapterLine.current.style.display = chapter?.title ? "block" : "none";
    }
    if (timeLine.current) timeLine.current.textContent = formatTime(t);
    if (thumbsOnRef.current && loader.current) {
      const step = bucketStep(dur);
      const bucket = Math.min(Math.floor(t / step), Math.floor(dur / step));
      const url = thumbUrl(keyRef.current, step, bucket);
      if (url !== wantedUrl.current) {
        loader.current.src = url;
        wantedUrl.current = loader.current.src;
      }
    }
    card.current.style.opacity = "1";
    // Keep the card inside the window (edge clamping, no layout shift).
    const half = card.current.offsetWidth / 2;
    const px = Math.min(window.innerWidth - 8 - half, Math.max(8 + half, x));
    card.current.style.left = `${px - rect.left}px`;
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
  const thick = waveMode ? "h-7" : active ? "h-3" : "h-2";

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
        className={`relative w-full overflow-hidden rounded-control transition-[height] duration-(--dur-fast) ease-out ${thick} ${
          waveMode ? "bg-raised" : "bg-track"
        }`}
      >
        {waveMode && wave ? (
          <WaveformTrack wave={wave} duration={dur} playedClip={playedClip} />
        ) : (
          <>
            <div ref={buffered} className="absolute inset-0 origin-left bg-track-buffer" />
            <div ref={played} className="absolute inset-0 origin-left bg-accent" />
          </>
        )}
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
        {waveformOn && progress !== null && (
          <div
            role="progressbar"
            aria-label="Analysing audio"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(progress * 100)}
            className="absolute bottom-0 left-0 h-px bg-fg"
            style={{ width: `${progress * 100}%` }}
          />
        )}
      </div>
      <div
        ref={ghostLine}
        className={`pointer-events-none absolute top-1/2 w-px -translate-y-1/2 bg-fg opacity-0 ${
          waveMode ? "h-7" : "h-3"
        }`}
      />
      <div
        ref={thumb}
        className={`pointer-events-none absolute top-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-fg transition-opacity duration-(--dur-fast) ${
          active && hasMedia && !waveMode ? "opacity-100" : "opacity-0"
        }`}
      />
      <div
        ref={card}
        className="pointer-events-none absolute bottom-full mb-3 flex -translate-x-1/2 flex-col items-center gap-1 rounded-surface border border-line-strong bg-raised p-1 opacity-0 shadow-float"
      >
        <div
          ref={picture}
          className="hidden h-33.75 w-60 overflow-hidden rounded-control border border-line-strong bg-video"
        >
          <img ref={image} alt="" className="h-full w-full object-contain" />
        </div>
        <div ref={chapterLine} className="hidden max-w-60 truncate px-1 text-fg-2 text-sm" />
        <div ref={timeLine} className="tabular px-1 text-base text-fg" />
      </div>
    </div>
  );
}
