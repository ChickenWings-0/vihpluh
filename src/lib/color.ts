/**
 * Accent colour derivation (DESIGN_SYSTEM §2.3). From one hex colour: hover and pressed
 * shades (OKLCH lightness ± a step), the translucent "muted" tint, an accent-coloured text
 * colour that stays readable on the surface, the text colour to put on an accent fill, and a
 * focus-ring colour that never disappears into the surface.
 * Contrast is WCAG 2 relative-luminance contrast; text must reach 4.5:1 (§13).
 */

export type Rgb = [r: number, g: number, b: number];
/** Lightness 0–1, chroma ≥ 0, hue in degrees. */
export type Oklch = [l: number, c: number, h: number];

export const ACCENT_SWATCHES: { name: string; hex: string }[] = [
  { name: "Cone Orange", hex: "#FF6B00" },
  { name: "Amber", hex: "#F5A300" },
  { name: "Red", hex: "#E5484D" },
  { name: "Pink", hex: "#E93D82" },
  { name: "Violet", hex: "#8E4EC6" },
  { name: "Blue", hex: "#3E8BFF" },
  { name: "Teal", hex: "#12A594" },
  { name: "Green", hex: "#46A758" },
];

export const DEFAULT_ACCENT = "#FF6B00";
const ON_ACCENT_DARK = "#0D0D0F";
const ON_ACCENT_LIGHT = "#FFFFFF";
const MIN_TEXT_CONTRAST = 4.5;
/** Control boundaries and the focus ring against their surface (§13). */
const MIN_EDGE_CONTRAST = 3;
/**
 * Accent fills (slider fills, toggle tracks, the played part of the seek bar) against the
 * surface. Lower than the edge rule on purpose: it catches an accent that vanishes into the
 * theme without recolouring the light ones the swatches include (Amber is 2.1:1 on white).
 */
const MIN_FILL_CONTRAST = 2;

export function isHex(text: string): boolean {
  return /^#[0-9a-f]{6}$/i.test(text);
}

export function parseHex(hex: string): Rgb | null {
  if (!isHex(hex)) return null;
  const n = Number.parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function toHex([r, g, b]: Rgb): string {
  const part = (v: number) =>
    Math.round(Math.min(255, Math.max(0, v)))
      .toString(16)
      .padStart(2, "0");
  return `#${part(r)}${part(g)}${part(b)}`.toUpperCase();
}

const toLinear = (v: number) => {
  const s = v / 255;
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};
const fromLinear = (v: number) => {
  const s = v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055;
  return s * 255;
};

/** sRGB → OKLCH (Björn Ottosson's OKLab, in cylindrical form). */
export function rgbToOklch(rgb: Rgb): Oklch {
  const [r, g, b] = rgb.map(toLinear) as Rgb;
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  const hue = (Math.atan2(B, A) * 180) / Math.PI;
  return [L, Math.hypot(A, B), hue < 0 ? hue + 360 : hue];
}

function oklchToLinear([L, C, h]: Oklch): Rgb {
  const A = C * Math.cos((h * Math.PI) / 180);
  const B = C * Math.sin((h * Math.PI) / 180);
  const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3;
  const m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3;
  const s = (L - 0.0894841775 * A - 1.291485548 * B) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}

const inGamut = (rgb: Rgb) => rgb.every((v) => v >= -0.0005 && v <= 1.0005);

/** OKLCH → sRGB. A colour outside sRGB keeps its lightness and hue and gives up chroma. */
export function oklchToRgb([L, C, h]: Oklch): Rgb {
  const light = Math.min(1, Math.max(0, L));
  let linear = oklchToLinear([light, C, h]);
  if (!inGamut(linear)) {
    let lo = 0;
    let hi = C;
    for (let i = 0; i < 24; i++) {
      const mid = (lo + hi) / 2;
      if (inGamut(oklchToLinear([light, mid, h]))) lo = mid;
      else hi = mid;
    }
    linear = oklchToLinear([light, lo, h]);
  }
  return linear.map((v) => fromLinear(Math.min(1, Math.max(0, v)))) as Rgb;
}

export function luminance(rgb: Rgb): number {
  const [r, g, b] = rgb.map(toLinear) as Rgb;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio of two colours, 1–21. */
export function contrast(a: Rgb, b: Rgb): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

/** The colour with its OKLCH lightness moved by `delta`. */
export function shiftLightness(rgb: Rgb, delta: number): Rgb {
  const [l, c, h] = rgbToOklch(rgb);
  return oklchToRgb([l + delta, c, h]);
}

/**
 * The accent as a text colour on `surface`: its lightness is moved (up on a dark surface,
 * down on a light one) only as far as needed to reach 4.5:1.
 */
export function readableOn(accent: Rgb, surface: Rgb, min = MIN_TEXT_CONTRAST): Rgb {
  if (contrast(accent, surface) >= min) return accent;
  const [l, c, h] = rgbToOklch(accent);
  const direction = luminance(surface) < 0.5 ? 1 : -1;
  for (let step = 1; step <= 100; step++) {
    const candidate = oklchToRgb([l + direction * step * 0.01, c, h]);
    if (contrast(candidate, surface) >= min) return candidate;
  }
  return direction > 0 ? [255, 255, 255] : [0, 0, 0];
}

export interface AccentTokens {
  "--accent": string;
  "--accent-hover": string;
  "--accent-active": string;
  "--accent-muted": string;
  "--accent-text": string;
  "--on-accent": string;
  "--focus-ring": string;
}

/** The surface accent-coloured text sits on, per theme (`--bg-surface`). */
const SURFACE: Record<"dark" | "light", Rgb> = { dark: [22, 22, 26], light: [255, 255, 255] };

/** Every accent-derived token for a colour and theme (§2.3). A bad colour gives the default's. */
export function deriveAccent(hex: string, theme: "dark" | "light"): AccentTokens {
  const chosen = parseHex(hex) ?? (parseHex(DEFAULT_ACCENT) as Rgb);
  // A pale accent on the light theme (or a dark one on the dark theme) is moved just far
  // enough to show as a fill; everything else derives from that.
  const accent = readableOn(chosen, SURFACE[theme], MIN_FILL_CONTRAST);
  const [r, g, b] = accent;
  const dark = parseHex(ON_ACCENT_DARK) as Rgb;
  const text = toHex(readableOn(accent, SURFACE[theme]));
  return {
    "--accent": toHex(accent),
    "--accent-hover": toHex(shiftLightness(accent, 0.06)),
    "--accent-active": toHex(shiftLightness(accent, -0.07)),
    "--accent-muted": `rgb(${r} ${g} ${b} / ${theme === "dark" ? 0.16 : 0.14})`,
    "--accent-text": text,
    "--on-accent": contrast(dark, accent) >= MIN_TEXT_CONTRAST ? ON_ACCENT_DARK : ON_ACCENT_LIGHT,
    // The ring is the accent itself while it stands out from the surface; a pale accent on
    // white (or a dark one on the dark theme) would vanish, so the readable shade steps in.
    "--focus-ring": contrast(accent, SURFACE[theme]) >= MIN_EDGE_CONTRAST ? toHex(accent) : text,
  };
}
