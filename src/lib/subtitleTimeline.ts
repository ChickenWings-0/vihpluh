/**
 * Maths for the subtitle timeline (DESIGN_SYSTEM §10.1): the time ↔ pixel mapping around a
 * fixed playhead, waveform columns for a time window, speech-onset detection, and snapping a
 * dragged delay so a cue start lands on an onset. Pure, so it is unit-tested without a canvas.
 */
import type { Cue } from "@/ipc/bindings/Cue";
import type { Waveform } from "./waveform";

/** Half-widths of the visible window, in seconds (zoom steps). */
export const ZOOM_SPANS = [4, 8, 15, 30] as const;
export const DEFAULT_SPAN = 8;

export interface View {
  /** Playhead time, seconds. */
  position: number;
  /** Seconds shown on each side of the playhead. */
  span: number;
  /** Width in CSS pixels. */
  width: number;
}

export const pxPerSecond = (view: View) => view.width / (2 * view.span);
export const timeToX = (view: View, time: number) =>
  view.width / 2 + (time - view.position) * pxPerSecond(view);
export const xToTime = (view: View, x: number) =>
  view.position + (x - view.width / 2) / pxPerSecond(view);

/** Subtitle delay after dragging the cue lane `dx` pixels (right = later). */
export function delayAfterDrag(startDelayMs: number, dx: number, view: View): number {
  return Math.round(startDelayMs + (dx / pxPerSecond(view)) * 1000);
}

/**
 * (min, max) pairs in −1…1 for each pixel column of the view. Columns outside the file are
 * silent. Unlike the seek bar's binning this takes a time window, not the whole file.
 */
export function windowPeaks(wave: Waveform, view: View): Float32Array {
  const columns = Math.max(0, Math.floor(view.width));
  const out = new Float32Array(columns * 2);
  const count = wave.peaks.length / 2;
  if (count === 0) return out;
  const perColumn = wave.peaksPerSecond / pxPerSecond(view);
  const first = (view.position - view.span) * wave.peaksPerSecond;
  for (let c = 0; c < columns; c++) {
    const from = Math.floor(first + c * perColumn);
    const to = Math.max(from + 1, Math.floor(first + (c + 1) * perColumn));
    let min = 0;
    let max = 0;
    for (let i = Math.max(0, from); i < to && i < count; i++) {
      const lo = wave.peaks[i * 2] ?? 0;
      const hi = wave.peaks[i * 2 + 1] ?? 0;
      if (lo < min) min = lo;
      if (hi > max) max = hi;
    }
    out[c * 2] = min / 128;
    out[c * 2 + 1] = max / 127;
  }
  return out;
}

/**
 * Times (seconds) where sound starts after a quiet stretch — where speech tends to begin.
 * A frame is loud at ≥ 25 % of the file's loudest moment; an onset is a loud frame whose
 * previous 120 ms were all below half of that.
 */
export function findOnsets(wave: Waveform): number[] {
  const count = wave.peaks.length / 2;
  const level = new Float32Array(count);
  let loudest = 0;
  for (let i = 0; i < count; i++) {
    const lo = -(wave.peaks[i * 2] ?? 0) / 128;
    const hi = (wave.peaks[i * 2 + 1] ?? 0) / 127;
    const a = Math.max(lo, hi);
    level[i] = a;
    if (a > loudest) loudest = a;
  }
  if (loudest < 0.02) return [];
  const loud = loudest * 0.25;
  const quiet = loud * 0.5;
  const lookback = Math.max(1, Math.round(wave.peaksPerSecond * 0.12));
  const onsets: number[] = [];
  let quietRun = lookback; // the start of the file counts as silence
  for (let i = 0; i < count; i++) {
    const a = level[i] ?? 0;
    if (a >= loud && quietRun >= lookback) onsets.push(i / wave.peaksPerSecond);
    quietRun = a < quiet ? quietRun + 1 : 0;
  }
  return onsets;
}

/** The value in sorted `values` nearest to `target`, or null when the list is empty. */
export function nearest(values: readonly number[], target: number): number | null {
  if (values.length === 0) return null;
  let lo = 0;
  let hi = values.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if ((values[mid] ?? 0) < target) lo = mid + 1;
    else hi = mid;
  }
  const after = values[lo] ?? 0;
  const before = values[lo - 1];
  return before !== undefined && target - before <= after - target ? before : after;
}

/**
 * Shift-snap: adjusts `delayMs` so that the cue start nearest to an onset lands exactly on
 * it. Only cues inside the view are considered, and only a move of at most `radiusMs`.
 */
export function snapDelay(
  delayMs: number,
  cues: readonly Cue[],
  onsets: readonly number[],
  view: View,
  radiusMs = 250,
): number {
  let best: number | null = null;
  const from = view.position - view.span;
  const to = view.position + view.span;
  for (const cue of cues) {
    const shown = cue.start + delayMs / 1000;
    if (shown < from || shown > to) continue;
    const onset = nearest(onsets, shown);
    if (onset === null) continue;
    const move = (onset - shown) * 1000;
    if (Math.abs(move) <= radiusMs && (best === null || Math.abs(move) < Math.abs(best))) {
      best = move;
    }
  }
  return best === null ? delayMs : Math.round(delayMs + best);
}

export interface Block {
  x: number;
  width: number;
  text: string | null;
  /** The playhead is inside this cue. */
  current: boolean;
}

/** Cue blocks to draw: shifted by the delay, clipped to cues that touch the view. */
export function layoutCues(cues: readonly Cue[], delayMs: number, view: View): Block[] {
  const shift = delayMs / 1000;
  const blocks: Block[] = [];
  for (const cue of cues) {
    const start = cue.start + shift;
    const end = cue.end + shift;
    const x = timeToX(view, start);
    const right = timeToX(view, end);
    if (right < 0 || x > view.width) continue;
    blocks.push({
      x,
      width: Math.max(2, right - x),
      text: cue.text,
      current: view.position >= start && view.position < end,
    });
  }
  return blocks;
}
