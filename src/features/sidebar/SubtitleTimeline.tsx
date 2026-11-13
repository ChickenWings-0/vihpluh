/**
 * The subtitle timeline (DESIGN_SYSTEM §10.1): the audio waveform on top and the subtitle
 * cues below, scrolling under a fixed playhead. Dragging the cue lane shifts every cue — that
 * is, changes the subtitle delay live; holding Shift snaps a cue start to where sound begins.
 * Dragging the waveform scrubs playback.
 *
 * Both lanes are drawn once into strips three views wide and then only moved with a
 * transform each frame (DESIGN_SYSTEM §12: animate transform, not paint). Repainting a
 * canvas every frame halved the webview's frame rate over video. A strip is redrawn when
 * the playhead nears its edge, when its data changes, or when another cue becomes current.
 */
import { Minus, Plus, SkipBack, SkipForward } from "lucide-react";
import { type PointerEvent, useEffect, useMemo, useRef, useState } from "react";
import { dispatch } from "@/actions";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import type { Coverage } from "@/ipc/bindings/Coverage";
import type { Cue } from "@/ipc/bindings/Cue";
import { playerSeek, subGetCues, subSetDelay } from "@/ipc/commands";
import { onCues } from "@/ipc/events";
import {
  currentCueStart,
  DEFAULT_SPAN,
  delayAfterDrag,
  findOnsets,
  layoutCues,
  pxPerSecond,
  STRIP_FACTOR,
  snapDelay,
  stripCovers,
  stripOffset,
  stripView,
  type View,
  windowPeaks,
  ZOOM_SPANS,
} from "@/lib/subtitleTimeline";
import { useAnimationFrame } from "@/lib/useAnimationFrame";
import { loudest } from "@/lib/waveform";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { getDuration, getPosition } from "@/stores/time";
import { useUiStore } from "@/stores/ui";
import { useWaveform } from "../chrome/useWaveform";

const WAVE_HEIGHT = 56;
const LANE_TOP = 60;
const LANE_HEIGHT = 32;
const DELAY_LIMIT_MS = 600_000;
const SEND_EVERY_MS = 30;
const SCRUB_EVERY_MS = 50;

interface CueState {
  cues: Cue[];
  coverage: Coverage;
}
const NO_CUES: CueState = { cues: [], coverage: "none" };

/** Every known cue of the selected track; refreshed when the core's index grows. */
function useCues(key: string, trackId: number | null): CueState {
  const [state, setState] = useState<CueState>(NO_CUES);
  useEffect(() => {
    setState(NO_CUES);
    if (key === "" || trackId === null) return;
    let cancelled = false;
    const load = () =>
      void subGetCues(trackId, 0, 1e9)
        .then((w) => {
          if (!cancelled) setState({ cues: w.cues, coverage: w.coverage });
        })
        .catch(() => undefined);
    load();
    const unlisten = onCues((e) => {
      if (e.mediaKey === key && e.trackId === trackId) load();
    });
    return () => {
      cancelled = true;
      unlisten();
    };
  }, [key, trackId]);
  return state;
}

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

interface Palette {
  wave: string;
  block: string;
  blockBorder: string;
  current: string;
  accent: string;
  text: string;
  font: string;
}

function readPalette(): Palette {
  return {
    wave: cssVar("--text-muted"),
    block: cssVar("--bg-active"),
    blockBorder: cssVar("--border-strong"),
    current: cssVar("--accent-muted"),
    accent: cssVar("--accent"),
    text: cssVar("--text-primary"),
    font: `11px ${cssVar("--font-ui") || "sans-serif"}`,
  };
}

/** Sizes a strip canvas for the device pixel ratio and returns a context in CSS pixels. */
function prepare(canvas: HTMLCanvasElement, width: number, height: number) {
  const dpr = window.devicePixelRatio || 1;
  const [w, h] = [Math.round(width * dpr), Math.round(height * dpr)];
  const ctx = canvas.getContext("2d");
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
  } else {
    // Same size: clearing is far cheaper than giving the canvas a new backing store.
    ctx?.setTransform(1, 0, 0, 1, 0, 0);
    ctx?.clearRect(0, 0, w, h);
  }
  ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
  return ctx;
}

/**
 * Moves a strip, snapped to whole device pixels and only when that changes. Every style
 * write costs the webview a composite over the video, and a shift of less than a pixel shows
 * nothing: at ±8 s the lanes move about 25 times a second, not on every frame.
 */
function move(canvas: HTMLCanvasElement | null, offset: number): void {
  if (!canvas) return;
  const dpr = window.devicePixelRatio || 1;
  const snapped = String(Math.round(offset * dpr) / dpr);
  if (canvas.dataset.offset === snapped) return;
  canvas.dataset.offset = snapped;
  canvas.style.transform = `translate3d(${snapped}px,0,0)`;
}

type Drag =
  | { kind: "cues"; startX: number; startDelay: number }
  | { kind: "scrub"; startX: number; startPos: number };

/** What a strip was last drawn with; a mismatch means it must be drawn again. */
interface Drawn {
  at: number;
  width: number;
  span: number;
  delay: number;
  current: number | null;
}

export function SubtitleTimeline() {
  const key = usePlayer((s) => s.media?.key ?? "") ?? "";
  const trackId = usePlayer((s) => s.selected.sub) ?? null;
  const delayMs = usePlayer((s) => s.subDelayMs) ?? 0;
  const theme = useConfig((c) => c.interface.theme);
  const accent = useConfig((c) => c.interface.accent);
  const hold = useUiStore((s) => s.hold);
  const [span, setSpan] = useState<number>(DEFAULT_SPAN);
  const { cues, coverage } = useCues(key, trackId);
  const { wave, progress } = useWaveform(true);
  const onsets = useMemo(() => (wave ? findOnsets(wave) : []), [wave]);
  const peakScale = useMemo(() => (wave ? 1 / Math.max(0.05, loudest(wave)) : 1), [wave]);

  const box = useRef<HTMLDivElement>(null);
  const waveCanvas = useRef<HTMLCanvasElement>(null);
  const cueCanvas = useRef<HTMLCanvasElement>(null);
  const width = useRef(0);
  const palette = useRef<Palette | null>(null);
  const waveDrawn = useRef<Drawn | null>(null);
  const cueDrawn = useRef<Drawn | null>(null);
  const drag = useRef<Drag | null>(null);
  /** The delay being dragged; shown until the core's echo catches up. */
  const draftDelay = useRef<{ ms: number; at: number } | null>(null);
  const scrubPos = useRef<number | null>(null);
  const lastSend = useRef(0);

  // Re-read the tokens when theme or accent changes (next frame: they are rewritten first).
  // biome-ignore lint/correctness/useExhaustiveDependencies: theme and accent are the triggers
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      palette.current = readPalette();
      waveDrawn.current = null;
      cueDrawn.current = null;
    });
    return () => cancelAnimationFrame(frame);
  }, [theme, accent]);

  // New data: draw the strips again on the next frame.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the data is the trigger
  useEffect(() => {
    waveDrawn.current = null;
  }, [wave]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: the data is the trigger
  useEffect(() => {
    cueDrawn.current = null;
  }, [cues]);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const observer = new ResizeObserver(() => {
      width.current = el.clientWidth;
    });
    observer.observe(el);
    width.current = el.clientWidth;
    return () => observer.disconnect();
  }, []);

  const shownDelay = () => {
    const draft = draftDelay.current;
    if (!draft) return delayMs;
    const settled = !drag.current && (draft.ms === delayMs || performance.now() - draft.at > 600);
    if (settled) {
      draftDelay.current = null;
      return delayMs;
    }
    return draft.ms;
  };

  const view = (): View => ({
    position: scrubPos.current ?? getPosition(),
    span,
    width: Math.max(1, width.current),
  });

  const drawWave = (v: View) => {
    const el = waveCanvas.current;
    const colors = palette.current;
    if (!el || !colors) return;
    const strip = stripView(v);
    const ctx = prepare(el, strip.width, WAVE_HEIGHT);
    if (!ctx) return;
    const mid = WAVE_HEIGHT / 2;
    ctx.fillStyle = colors.wave;
    if (!wave) {
      ctx.fillRect(0, mid, strip.width, 1);
      return;
    }
    const columns = windowPeaks(wave, strip);
    const half = mid - 4;
    for (let c = 0; c < columns.length / 2; c++) {
      const hi = Math.min(1, (columns[c * 2 + 1] ?? 0) * peakScale);
      const lo = Math.max(-1, (columns[c * 2] ?? 0) * peakScale);
      ctx.fillRect(c, mid - hi * half, 1, Math.max(1, (hi - lo) * half));
    }
  };

  const drawCues = (v: View, delay: number, now: number) => {
    const el = cueCanvas.current;
    const colors = palette.current;
    if (!el || !colors) return;
    const strip = stripView(v);
    const ctx = prepare(el, strip.width, LANE_HEIGHT);
    if (!ctx) return;
    ctx.font = colors.font;
    ctx.textBaseline = "middle";
    for (const block of layoutCues(cues, delay, strip, now)) {
      ctx.beginPath();
      ctx.roundRect(block.x + 0.5, 0.5, block.width - 1, LANE_HEIGHT - 1, 4);
      ctx.fillStyle = block.current ? colors.current : colors.block;
      ctx.fill();
      ctx.strokeStyle = block.current ? colors.accent : colors.blockBorder;
      ctx.lineWidth = 1;
      ctx.stroke();
      if (block.text && block.width > 16) {
        ctx.save();
        ctx.clip();
        ctx.fillStyle = colors.text;
        ctx.fillText(block.text.replace(/\n/g, " "), Math.max(block.x, 0) + 5, LANE_HEIGHT / 2);
        ctx.restore();
      }
    }
  };

  useAnimationFrame(() => {
    const w = width.current;
    if (!palette.current || w <= 0) return;
    const v = view();
    const now = v.position;

    // At most one strip is redrawn per frame: the two together make a long frame, and
    // `stripCovers` leaves a quarter of the span in hand for the one that waits.
    let redrew = false;
    const waveState = waveDrawn.current;
    const waveFits =
      waveState !== null &&
      waveState.width === w &&
      waveState.span === span &&
      stripCovers(v, waveState.at, now);
    if (!waveFits) {
      drawWave(v);
      redrew = true;
      waveDrawn.current = { at: now, width: w, span, delay: 0, current: null };
    }
    const waveAt = waveDrawn.current?.at ?? now;
    move(waveCanvas.current, stripOffset(v, waveAt, now));

    // The cue strip is drawn with the delay of that moment; a different delay since then is
    // only a further shift, so dragging the lane moves it without repainting.
    const delay = shownDelay();
    const current = currentCueStart(cues, delay, now);
    const cueState = cueDrawn.current;
    const shifted = (state: Drawn) => now - (delay - state.delay) / 1000;
    const cuesFit =
      cueState !== null &&
      cueState.width === w &&
      cueState.span === span &&
      cueState.current === current &&
      stripCovers(v, cueState.at, shifted(cueState));
    if (!cuesFit && (!redrew || cueState === null)) {
      drawCues(v, delay, now);
      cueDrawn.current = { at: now, width: w, span, delay, current };
    }
    const drawn = cueDrawn.current;
    if (drawn) move(cueCanvas.current, stripOffset(v, drawn.at, shifted(drawn)));
  });

  const localX = (e: PointerEvent<HTMLDivElement>) =>
    e.clientX - e.currentTarget.getBoundingClientRect().left;
  const localY = (e: PointerEvent<HTMLDivElement>) =>
    e.clientY - e.currentTarget.getBoundingClientRect().top;

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || key === "") return;
    e.preventDefault();
    const inLane = localY(e) >= LANE_TOP - 2;
    if (inLane && trackId === null) return;
    drag.current = inLane
      ? { kind: "cues", startX: localX(e), startDelay: shownDelay() }
      : { kind: "scrub", startX: localX(e), startPos: getPosition() };
    e.currentTarget.setPointerCapture(e.pointerId);
    hold("subtitle-timeline", true);
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) {
      e.currentTarget.style.cursor = localY(e) >= LANE_TOP - 2 ? "ew-resize" : "grab";
      return;
    }
    const dx = localX(e) - d.startX;
    const v = view();
    const now = performance.now();
    if (d.kind === "cues") {
      let ms = delayAfterDrag(d.startDelay, dx, v);
      if (e.shiftKey) ms = snapDelay(ms, cues, onsets, v);
      ms = Math.max(-DELAY_LIMIT_MS, Math.min(DELAY_LIMIT_MS, ms));
      draftDelay.current = { ms, at: now };
      if (now - lastSend.current >= SEND_EVERY_MS) {
        lastSend.current = now;
        void subSetDelay(ms).catch(reportError);
      }
    } else {
      const duration = getDuration();
      const target = Math.max(0, Math.min(duration || 0, d.startPos - dx / pxPerSecond(v)));
      scrubPos.current = target;
      if (now - lastSend.current >= SCRUB_EVERY_MS) {
        lastSend.current = now;
        void playerSeek({ kind: "absolute", value: target, exact: false }).catch(reportError);
      }
    }
  };

  const finish = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    hold("subtitle-timeline", false);
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    if (d.kind === "cues") {
      const draft = draftDelay.current;
      if (draft) {
        draft.at = performance.now();
        void subSetDelay(draft.ms).catch(reportError);
      }
    } else if (scrubPos.current !== null) {
      const target = scrubPos.current;
      void playerSeek({ kind: "absolute", value: target, exact: true }).catch(reportError);
      // Keep showing the target until the seek lands.
      setTimeout(() => {
        scrubPos.current = null;
      }, 250);
    }
  };

  const zoom = (dir: 1 | -1) => {
    const index = ZOOM_SPANS.indexOf(span as (typeof ZOOM_SPANS)[number]);
    const next = ZOOM_SPANS[Math.max(0, Math.min(ZOOM_SPANS.length - 1, index + dir))];
    if (next !== undefined) setSpan(next);
  };

  const description =
    trackId === null
      ? "Subtitle timeline: no subtitle track selected"
      : `Subtitle timeline: ${cues.length} lines, ${span} seconds each side of now. Drag the lines to change the subtitle delay; hold Shift to snap to speech.`;
  const unknown = trackId !== null && coverage !== "full";
  const stripStyle = { width: `${STRIP_FACTOR * 100}%`, willChange: "transform" } as const;

  return (
    <div className="flex flex-col gap-1.5">
      <div
        ref={box}
        role="img"
        aria-label={description}
        data-timeline
        data-coverage={coverage}
        className="relative h-24 touch-none overflow-hidden rounded-control bg-raised"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={finish}
        onPointerCancel={finish}
        onLostPointerCapture={finish}
      >
        <canvas
          ref={waveCanvas}
          className="pointer-events-none absolute top-0 left-0 h-14"
          style={stripStyle}
        />
        {/* Where the cue index is incomplete, a hatch says "not known yet" (§11.3). */}
        <div
          className={`pointer-events-none absolute inset-x-0 top-15 h-8 ${unknown ? "timeline-hatch" : ""}`}
        />
        <canvas
          ref={cueCanvas}
          className="pointer-events-none absolute top-15 left-0 h-8"
          style={stripStyle}
        />
        <div className="pointer-events-none absolute inset-y-0 left-1/2 w-px bg-accent" />
        {!wave && progress !== null && (
          <span className="tabular pointer-events-none absolute top-1 left-2 text-fg-3 text-xs">
            Analysing audio… {Math.round(progress * 100)} %
          </span>
        )}
      </div>
      <div className="flex items-center gap-1">
        <Button
          size="sm"
          icon={SkipBack}
          disabled={trackId === null}
          title="Shift the subtitles so the previous line shows now"
          onClick={() => void dispatch("subtitles.sync_previous_cue", { trigger: "button" })}
        >
          Prev to now
        </Button>
        <Button
          size="sm"
          icon={SkipForward}
          disabled={trackId === null}
          title="Shift the subtitles so the next line shows now"
          onClick={() => void dispatch("subtitles.sync_next_cue", { trigger: "button" })}
        >
          Next to now
        </Button>
        <span className="tabular flex-1 text-right text-fg-3 text-xs">±{span} s</span>
        <IconButton
          icon={Minus}
          label="Zoom out"
          size="panel"
          disabled={span === ZOOM_SPANS[ZOOM_SPANS.length - 1]}
          onClick={() => zoom(1)}
        />
        <IconButton
          icon={Plus}
          label="Zoom in"
          size="panel"
          disabled={span === ZOOM_SPANS[0]}
          onClick={() => zoom(-1)}
        />
      </div>
    </div>
  );
}
