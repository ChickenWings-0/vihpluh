/** Formats seconds as `m:ss`, or `h:mm:ss` from one hour (DESIGN_SYSTEM §7.4). */
export function formatTime(seconds: number): string {
  const total = Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0;
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/** `m:ss.mmm` (or `h:mm:ss.mmm`) for exact positions such as A–B loop points. */
export function formatTimePrecise(seconds: number): string {
  const safe = Number.isFinite(seconds) && seconds > 0 ? seconds : 0;
  const ms = Math.round((safe % 1) * 1000);
  const whole = Math.floor(safe) + (ms === 1000 ? 1 : 0);
  return `${formatTime(whole)}.${String(ms % 1000).padStart(3, "0")}`;
}

/** Parses `[h:]m:ss[.fff]` or plain seconds; null when the text is not a time. */
export function parseTime(text: string): number | null {
  const trimmed = text.trim();
  if (!/^\d+(:\d{1,2}){0,2}(\.\d+)?$/.test(trimmed)) return null;
  const parts = trimmed.split(":").map(Number);
  let seconds = 0;
  for (const part of parts) seconds = seconds * 60 + part;
  return Number.isFinite(seconds) ? seconds : null;
}
