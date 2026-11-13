/**
 * `.vwf` waveform files (ARCHITECTURE §11.2) and binning them to pixel columns for the seek
 * bar. Layout (little-endian): "VWF1", u32 peaks per second, u32 peak count, f32 duration,
 * then `count × { i8 min, i8 max }`.
 */
export interface Waveform {
  peaksPerSecond: number;
  duration: number;
  /** Interleaved min, max pairs (−128…127). */
  peaks: Int8Array;
}

export function parseVwf(buffer: ArrayBuffer): Waveform | null {
  if (buffer.byteLength < 16) return null;
  const view = new DataView(buffer);
  const magic = String.fromCharCode(
    view.getUint8(0),
    view.getUint8(1),
    view.getUint8(2),
    view.getUint8(3),
  );
  if (magic !== "VWF1") return null;
  const peaksPerSecond = view.getUint32(4, true);
  const count = view.getUint32(8, true);
  const duration = view.getFloat32(12, true);
  if (buffer.byteLength < 16 + count * 2) return null;
  return { peaksPerSecond, duration, peaks: new Int8Array(buffer, 16, count * 2) };
}

/** The loudest absolute peak in the file, 0–1 (for scaling the drawing). */
export function loudest(wave: Waveform): number {
  let max = 0;
  for (const v of wave.peaks) {
    const a = v < 0 ? -v / 128 : v / 127;
    if (a > max) max = a;
  }
  return max;
}

/**
 * Reduces the peaks to `columns` (min, max) pairs in −1…1, one per pixel column across
 * `duration` seconds (the player's duration, so the drawing lines up with the seek bar).
 */
export function binPeaks(wave: Waveform, columns: number, duration: number): Float32Array {
  const out = new Float32Array(columns * 2);
  const count = wave.peaks.length / 2;
  if (count === 0 || columns <= 0 || duration <= 0) return out;
  const perColumn = (duration * wave.peaksPerSecond) / columns;
  for (let c = 0; c < columns; c++) {
    const from = Math.floor(c * perColumn);
    const to = Math.max(from + 1, Math.floor((c + 1) * perColumn));
    let min = 127;
    let max = -128;
    for (let i = from; i < to && i < count; i++) {
      const lo = wave.peaks[i * 2] ?? 0;
      const hi = wave.peaks[i * 2 + 1] ?? 0;
      if (lo < min) min = lo;
      if (hi > max) max = hi;
    }
    if (max < min) {
      min = 0;
      max = 0;
    }
    out[c * 2] = min / 128;
    out[c * 2 + 1] = max / 127;
  }
  return out;
}
