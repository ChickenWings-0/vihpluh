import { describe, expect, it } from "vitest";
import type { EqState } from "@/ipc/bindings/EqState";
import { coefficients, GRAPHIC_FREQS, logFrequencies, magnitudeDb, response } from "./eq";

const peak = (freq: number, gainDb: number, q = 1) => ({ freq, gainDb, q, type: "peak" });

// Reference values computed independently from the RBJ cookbook formulas (fs = 48 kHz).
const close = (actual: number[], expected: number[]) => {
  actual.forEach((v, i) => {
    expect(v).toBeCloseTo(expected[i] ?? Number.NaN, 8);
  });
};

describe("biquad coefficients", () => {
  it("peaking, Q width", () => {
    close(
      coefficients(peak(1000, 6), false),
      [1.043953087, -1.895320724, 0.867722285, -1.895320724, 0.911675372],
    );
  });

  it("peaking, one-octave width (graphic mode)", () => {
    close(
      coefficients(peak(1000, 6), true),
      [1.031577524, -1.919976914, 0.904966795, -1.919976914, 0.936544319],
    );
  });

  it("low shelf", () => {
    close(
      coefficients({ freq: 100, gainDb: 6, q: 0.7, type: "lowshelf" }, false),
      [1.003250007, -1.984208759, 0.981198897, -1.984268653, 0.98438901],
    );
  });
});

describe("response", () => {
  it("a peaking band reaches its gain at the centre", () => {
    expect(magnitudeDb(coefficients(peak(1000, 6), true), 1000)).toBeCloseTo(6, 6);
    expect(magnitudeDb(coefficients(peak(1000, 6), true), 500)).toBeCloseTo(1.1374, 3);
  });

  it("shelves reach their gain at the far end", () => {
    const low = coefficients({ freq: 100, gainDb: 6, q: 0.7, type: "lowshelf" }, false);
    const high = coefficients({ freq: 8000, gainDb: -4, q: 0.7, type: "highshelf" }, false);
    expect(magnitudeDb(low, 10)).toBeCloseTo(6, 1);
    expect(magnitudeDb(high, 23000)).toBeCloseTo(-4, 1);
  });

  it("sums bands and preamp; off is flat", () => {
    const eq: EqState = {
      enabled: true,
      mode: "graphic10",
      preset: null,
      preampDb: -2,
      bands: GRAPHIC_FREQS.map((f) => peak(f, 0)),
    };
    expect(response(eq, [1000])[0]).toBeCloseTo(-2, 9);
    expect(response({ ...eq, enabled: false }, [1000])[0]).toBe(0);
  });

  it("log frequency axis", () => {
    const f = logFrequencies(3);
    expect(f[0]).toBeCloseTo(20);
    expect(f[1]).toBeCloseTo(632.46, 1);
    expect(f[2]).toBeCloseTo(20000);
  });
});
