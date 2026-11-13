/**
 * Equalizer response maths (DESIGN_SYSTEM §10.2): the curve drawn in the Audio tab is the
 * true summed response of the biquads the core sends to FFmpeg, not a spline through the
 * nodes. Formulas follow FFmpeg's af_biquads (RBJ cookbook): `equalizer` (peaking, octave or
 * Q width), `bass` (low shelf), `treble` (high shelf).
 */
import type { EqBand } from "@/ipc/bindings/EqBand";
import type { EqState } from "@/ipc/bindings/EqState";

export const SAMPLE_RATE = 48000;
export const GRAPHIC_FREQS = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

/** Normalised coefficients: [b0, b1, b2, a1, a2] with a0 = 1. */
export type Coefficients = [number, number, number, number, number];

export function coefficients(
  band: EqBand,
  octaveWidth: boolean,
  fs: number = SAMPLE_RATE,
): Coefficients {
  const A = 10 ** (band.gainDb / 40);
  const w0 = (2 * Math.PI * band.freq) / fs;
  const sin = Math.sin(w0);
  const cos = Math.cos(w0);
  const alpha = octaveWidth ? sin * Math.sinh(((Math.LN2 / 2) * 1 * w0) / sin) : sin / (2 * band.q);
  let b0: number;
  let b1: number;
  let b2: number;
  let a0: number;
  let a1: number;
  let a2: number;
  const sA = Math.sqrt(A);
  if (band.type === "lowshelf") {
    b0 = A * (A + 1 - (A - 1) * cos + 2 * sA * alpha);
    b1 = 2 * A * (A - 1 - (A + 1) * cos);
    b2 = A * (A + 1 - (A - 1) * cos - 2 * sA * alpha);
    a0 = A + 1 + (A - 1) * cos + 2 * sA * alpha;
    a1 = -2 * (A - 1 + (A + 1) * cos);
    a2 = A + 1 + (A - 1) * cos - 2 * sA * alpha;
  } else if (band.type === "highshelf") {
    b0 = A * (A + 1 + (A - 1) * cos + 2 * sA * alpha);
    b1 = -2 * A * (A - 1 + (A + 1) * cos);
    b2 = A * (A + 1 + (A - 1) * cos - 2 * sA * alpha);
    a0 = A + 1 - (A - 1) * cos + 2 * sA * alpha;
    a1 = 2 * (A - 1 - (A + 1) * cos);
    a2 = A + 1 - (A - 1) * cos - 2 * sA * alpha;
  } else {
    b0 = 1 + alpha * A;
    b1 = -2 * cos;
    b2 = 1 - alpha * A;
    a0 = 1 + alpha / A;
    a1 = -2 * cos;
    a2 = 1 - alpha / A;
  }
  return [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0];
}

/** |H(e^jw)| in dB for one biquad at frequency `f`. */
export function magnitudeDb(c: Coefficients, f: number, fs: number = SAMPLE_RATE): number {
  const w = (2 * Math.PI * f) / fs;
  const cos1 = Math.cos(w);
  const sin1 = Math.sin(w);
  const cos2 = Math.cos(2 * w);
  const sin2 = Math.sin(2 * w);
  const [b0, b1, b2, a1, a2] = c;
  const nr = b0 + b1 * cos1 + b2 * cos2;
  const ni = -(b1 * sin1 + b2 * sin2);
  const dr = 1 + a1 * cos1 + a2 * cos2;
  const di = -(a1 * sin1 + a2 * sin2);
  const num = nr * nr + ni * ni;
  const den = dr * dr + di * di;
  return 10 * Math.log10(num / den);
}

/** Summed response (bands + preamp) at each frequency, in dB. Off = flat 0 dB. */
export function response(eq: EqState, freqs: number[]): number[] {
  if (!eq.enabled) return freqs.map(() => 0);
  const octave = eq.mode === "graphic10";
  const filters = eq.bands.filter((b) => b.gainDb !== 0).map((b) => coefficients(b, octave));
  return freqs.map((f) => eq.preampDb + filters.reduce((sum, c) => sum + magnitudeDb(c, f), 0));
}

/** `count` frequencies spaced evenly on a log axis from `low` to `high` Hz. */
export function logFrequencies(count: number, low = 20, high = 20000): number[] {
  const span = Math.log10(high / low);
  return Array.from({ length: count }, (_, i) => low * 10 ** ((span * i) / (count - 1)));
}

/** Position (0–1) of a frequency on the 20 Hz – 20 kHz log axis, and back. */
export const freqToX = (f: number, low = 20, high = 20000) =>
  Math.log10(f / low) / Math.log10(high / low);
export const xToFreq = (x: number, low = 20, high = 20000) => low * (high / low) ** x;

export function formatFreq(f: number): string {
  return f >= 1000 ? `${Number((f / 1000).toFixed(f >= 10000 ? 0 : 1))}k` : `${Math.round(f)}`;
}
