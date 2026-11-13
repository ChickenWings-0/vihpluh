import { describe, expect, it } from "vitest";
import { binPeaks, loudest, parseVwf } from "./waveform";

function vwf(pairs: [number, number][], pps = 50, duration = pairs.length / 50): ArrayBuffer {
  const buffer = new ArrayBuffer(16 + pairs.length * 2);
  const view = new DataView(buffer);
  [..."VWF1"].forEach((ch, i) => {
    view.setUint8(i, ch.charCodeAt(0));
  });
  view.setUint32(4, pps, true);
  view.setUint32(8, pairs.length, true);
  view.setFloat32(12, duration, true);
  pairs.forEach(([lo, hi], i) => {
    view.setInt8(16 + i * 2, lo);
    view.setInt8(17 + i * 2, hi);
  });
  return buffer;
}

describe("vwf", () => {
  it("parses the header and peaks", () => {
    const wave = parseVwf(
      vwf([
        [-10, 20],
        [-128, 127],
      ]),
    );
    expect(wave?.peaksPerSecond).toBe(50);
    expect(wave?.duration).toBeCloseTo(0.04);
    expect([...(wave?.peaks ?? [])]).toEqual([-10, 20, -128, 127]);
  });

  it("rejects other files and truncated ones", () => {
    expect(parseVwf(new ArrayBuffer(8))).toBeNull();
    const bad = vwf([[0, 0]]);
    new DataView(bad).setUint8(3, 0x30);
    expect(parseVwf(bad)).toBeNull();
    expect(parseVwf(vwf([[0, 0]]).slice(0, 17))).toBeNull();
  });

  it("bins peaks to columns, keeping each column's extremes", () => {
    // 100 peaks over 2 s: a loud burst in the second quarter.
    const pairs: [number, number][] = Array.from({ length: 100 }, (_, i) =>
      i >= 25 && i < 50 ? [-100, 100] : [-5, 5],
    );
    const wave = parseVwf(vwf(pairs));
    if (!wave) throw new Error("parse failed");
    const bins = binPeaks(wave, 4, 2);
    expect(bins.length).toBe(8);
    expect(bins[2]).toBeCloseTo(-100 / 128);
    expect(bins[3]).toBeCloseTo(100 / 127);
    expect(bins[1]).toBeCloseTo(5 / 127);
  });

  it("finds the loudest peak", () => {
    const wave = parseVwf(
      vwf([
        [-64, 10],
        [-5, 127],
      ]),
    );
    if (!wave) throw new Error("parse failed");
    expect(loudest(wave)).toBe(1);
  });
});
