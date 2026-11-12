/**
 * Playback position (ARCHITECTURE §4.2, §7.4). Deliberately not React state: it changes every
 * frame. The core sends `player://time` at 5 Hz while playing (and at once on seek, pause, and
 * load); between events the position is extrapolated from the receive time and the speed.
 * Read it with `getPosition()` inside a requestAnimationFrame loop (see `useAnimationFrame`).
 */
import type { TimeEvent } from "@/ipc/bindings/TimeEvent";

export interface TimeSample {
  pos: number;
  dur: number;
  cacheEnd: number;
  /** `performance.now()` when the event arrived. */
  receivedAt: number;
}

let sample: TimeSample = { pos: 0, dur: 0, cacheEnd: 0, receivedAt: 0 };
let advancing = false;
let rate = 1;

export function receiveTime(event: TimeEvent, now: number = performance.now()): void {
  sample = { pos: event.pos, dur: event.dur, cacheEnd: event.cacheEnd, receivedAt: now };
}

/** Called when status or speed changes. `playing` means the position is moving forward. */
export function setClock(playing: boolean, speed: number, now: number = performance.now()): void {
  // Re-anchor so a change of rate never makes the position jump.
  sample = { ...sample, pos: getPosition(now), receivedAt: now };
  advancing = playing;
  rate = speed;
}

/** Pure interpolation, exported for tests. */
export function interpolate(
  s: TimeSample,
  now: number,
  isAdvancing: boolean,
  speed: number,
): number {
  if (!isAdvancing) return s.pos;
  const elapsed = Math.max(0, now - s.receivedAt) / 1000;
  const pos = s.pos + elapsed * speed;
  return s.dur > 0 ? Math.min(pos, s.dur) : pos;
}

export function getPosition(now: number = performance.now()): number {
  return interpolate(sample, now, advancing, rate);
}

export function getDuration(): number {
  return sample.dur;
}

export function getCacheEnd(): number {
  return sample.cacheEnd;
}

/** Test helper. */
export function resetTime(): void {
  sample = { pos: 0, dur: 0, cacheEnd: 0, receivedAt: 0 };
  advancing = false;
  rate = 1;
}

// Development builds expose the interpolation so verification tooling can measure drift
// against mpv's own `time-pos` (IMPLEMENTATION_PLAN §4).
if (import.meta.env.DEV && typeof window !== "undefined") {
  (window as unknown as { __vihpluhPositionAt: typeof getPosition }).__vihpluhPositionAt =
    getPosition;
}
