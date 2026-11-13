import { describe, expect, it } from "vitest";
import type { Cue } from "@/ipc/bindings/Cue";
import {
  currentCueStart,
  delayAfterDrag,
  findOnsets,
  layoutCues,
  nearest,
  snapDelay,
  stripCovers,
  stripOffset,
  stripView,
  timeToX,
  type View,
  windowPeaks,
  xToTime,
} from "./subtitleTimeline";
import type { Waveform } from "./waveform";

/** 50 peaks per second; loud between `on` and `off` seconds of every 4 s. */
function bursts(seconds: number, on = 1, off = 3): Waveform {
  const pps = 50;
  const peaks = new Int8Array(seconds * pps * 2);
  for (let i = 0; i < seconds * pps; i++) {
    const t = (i / pps) % 4;
    const loud = t >= on && t < off;
    peaks[i * 2] = loud ? -100 : -4;
    peaks[i * 2 + 1] = loud ? 100 : 4;
  }
  return { peaksPerSecond: pps, duration: seconds, peaks };
}

const view: View = { position: 10, span: 8, width: 320 };
const cue = (start: number, end: number, text = "x"): Cue => ({ start, end, text });

describe("time mapping", () => {
  it("puts the playhead at the centre and round-trips", () => {
    expect(timeToX(view, 10)).toBe(160);
    expect(timeToX(view, 2)).toBe(0);
    expect(timeToX(view, 18)).toBe(320);
    expect(xToTime(view, timeToX(view, 13.37))).toBeCloseTo(13.37, 9);
  });

  it("turns a drag distance into a delay", () => {
    // 20 px per second: 8 px to the right is 400 ms later.
    expect(delayAfterDrag(0, 8, view)).toBe(400);
    expect(delayAfterDrag(-350, -20, view)).toBe(-1350);
  });
});

describe("windowPeaks", () => {
  it("bins the visible window and leaves time outside the file silent", () => {
    const wave = bursts(12);
    const columns = windowPeaks(wave, { position: 2, span: 4, width: 80 });
    expect(columns.length).toBe(160);
    // Column 0 is t = −2 s (before the file): silent.
    expect(columns[1]).toBe(0);
    // t = 2 s (the centre) is inside a burst; t = 0.5 s is quiet.
    expect(columns[40 * 2 + 1]).toBeGreaterThan(0.7);
    expect(columns[25 * 2 + 1]).toBeLessThan(0.1);
  });
});

describe("findOnsets", () => {
  it("finds where each burst starts", () => {
    expect(findOnsets(bursts(12))).toEqual([1, 5, 9]);
  });

  it("finds nothing in silence", () => {
    const silent: Waveform = { peaksPerSecond: 50, duration: 2, peaks: new Int8Array(200) };
    expect(findOnsets(silent)).toEqual([]);
  });
});

describe("nearest", () => {
  it("picks the closer neighbour", () => {
    expect(nearest([1, 5, 9], 2.9)).toBe(1);
    expect(nearest([1, 5, 9], 3.1)).toBe(5);
    expect(nearest([1, 5, 9], 40)).toBe(9);
    expect(nearest([1, 5, 9], -3)).toBe(1);
    expect(nearest([], 1)).toBeNull();
  });
});

describe("snapDelay", () => {
  const onsets = [1, 5, 9, 13];
  // Subtitles authored 400 ms early: every line starts 0.6 s into its 4 s block.
  const cues = [cue(0.6, 3), cue(4.6, 7), cue(8.6, 11), cue(12.6, 15)];

  it("lands a cue start exactly on an onset when the drag is close", () => {
    expect(snapDelay(330, cues, onsets, view)).toBe(400);
    expect(snapDelay(520, cues, onsets, view)).toBe(400);
  });

  it("leaves the delay alone when nothing is within reach", () => {
    expect(snapDelay(1000, cues, onsets, view)).toBe(1000);
    expect(snapDelay(100, [], onsets, view)).toBe(100);
    expect(snapDelay(100, cues, [], view)).toBe(100);
  });
});

describe("layoutCues", () => {
  it("shifts by the delay, clips to the view, and marks the cue under the playhead", () => {
    const blocks = layoutCues([cue(-20, -19), cue(9, 11, "now"), cue(17.9, 30)], 0, view);
    expect(blocks.map((b) => b.text)).toEqual(["now", "x"]);
    expect(blocks[0]).toMatchObject({ x: 140, width: 40, current: true });
    // With +2 s of delay the same cue starts after the playhead.
    expect(layoutCues([cue(9, 11)], 2000, view)[0]).toMatchObject({ x: 180, current: false });
  });
});

describe("strips", () => {
  it("draws two views wide at the same scale", () => {
    const strip = stripView(view);
    expect(strip).toEqual({ position: 10, span: 16, width: 640 });
    // A time keeps its distance from the strip's centre: 20 px per second.
    expect(timeToX(strip, 12) - timeToX(strip, 10)).toBe(40);
  });

  it("shifts so the playhead time sits at the view centre", () => {
    // Drawn around t = 10: at t = 10 the strip's centre (320) must be at the view centre (160).
    expect(stripOffset(view, 10, 10)).toBe(-160);
    // One second later the strip has moved 20 px to the left.
    expect(stripOffset(view, 10, 11)).toBe(-180);
    const strip = stripView(view);
    expect(timeToX(strip, 11) + stripOffset(view, 10, 11)).toBe(160);
  });

  it("knows when it has to be drawn again", () => {
    // The strip reaches 16 s each way and the view needs 8: up to 6 s of travel is covered.
    expect(stripCovers(view, 10, 10)).toBe(true);
    expect(stripCovers(view, 10, 15.9)).toBe(true);
    expect(stripCovers(view, 10, 16.1)).toBe(false);
    expect(stripCovers(view, 10, 3)).toBe(false);
  });

  it("finds the cue under the playhead with the delay applied", () => {
    const cues = [cue(1, 3), cue(5, 7)];
    expect(currentCueStart(cues, 0, 2)).toBe(1);
    expect(currentCueStart(cues, 0, 4)).toBeNull();
    expect(currentCueStart(cues, 400, 5.2)).toBeNull();
    expect(currentCueStart(cues, 400, 5.5)).toBe(5);
  });
});
