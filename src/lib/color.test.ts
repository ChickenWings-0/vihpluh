import { describe, expect, it } from "vitest";
import {
  ACCENT_SWATCHES,
  contrast,
  deriveAccent,
  isHex,
  oklchToRgb,
  parseHex,
  type Rgb,
  readableOn,
  rgbToOklch,
  shiftLightness,
  toHex,
} from "./color";

const rgb = (hex: string) => parseHex(hex) as Rgb;
const DARK_SURFACE = rgb("#16161A");
const WHITE = rgb("#FFFFFF");

describe("hex", () => {
  it("parses and prints", () => {
    expect(parseHex("#FF6B00")).toEqual([255, 107, 0]);
    expect(parseHex("#ff6b00")).toEqual([255, 107, 0]);
    expect(toHex([255, 107, 0])).toBe("#FF6B00");
    expect(toHex([300, -5, 12.4])).toBe("#FF000C");
    for (const bad of ["", "FF6B00", "#FFF", "#GGGGGG", "#FF6B0000"]) {
      expect(isHex(bad), bad).toBe(false);
      expect(parseHex(bad)).toBeNull();
    }
  });
});

describe("OKLCH", () => {
  it("matches reference values", () => {
    // White and black are the ends of the lightness axis, with no chroma.
    expect(rgbToOklch([255, 255, 255])[0]).toBeCloseTo(1, 3);
    expect(rgbToOklch([255, 255, 255])[1]).toBeCloseTo(0, 3);
    expect(rgbToOklch([0, 0, 0])[0]).toBeCloseTo(0, 3);
    // sRGB red in OKLCH: L 0.628, C 0.2577, h 29.23°.
    const [l, c, h] = rgbToOklch([255, 0, 0]);
    expect(l).toBeCloseTo(0.628, 3);
    expect(c).toBeCloseTo(0.2577, 3);
    expect(h).toBeCloseTo(29.23, 1);
  });

  it("round-trips every swatch", () => {
    for (const { hex } of ACCENT_SWATCHES) {
      expect(toHex(oklchToRgb(rgbToOklch(rgb(hex)))), hex).toBe(hex);
    }
  });

  it("keeps hue and gives up chroma outside sRGB", () => {
    const vivid = oklchToRgb([0.9, 0.4, 145]);
    expect(vivid.every((v) => v >= 0 && v <= 255)).toBe(true);
    expect(rgbToOklch(vivid)[2]).toBeCloseTo(145, 0);
    expect(rgbToOklch(vivid)[0]).toBeCloseTo(0.9, 1);
  });

  it("moves lightness by the asked amount", () => {
    const base = rgbToOklch(rgb("#3E8BFF"))[0];
    expect(rgbToOklch(shiftLightness(rgb("#3E8BFF"), 0.06))[0]).toBeCloseTo(base + 0.06, 2);
    expect(rgbToOklch(shiftLightness(rgb("#3E8BFF"), -0.07))[0]).toBeCloseTo(base - 0.07, 2);
  });
});

describe("contrast", () => {
  it("matches WCAG's known ratios", () => {
    expect(contrast([0, 0, 0], WHITE)).toBeCloseTo(21, 1);
    expect(contrast(WHITE, WHITE)).toBe(1);
    // DESIGN_SYSTEM §2.1: dark text on the default orange passes, white on it fails.
    expect(contrast(rgb("#0D0D0F"), rgb("#FF6B00"))).toBeCloseTo(6.8, 1);
    expect(contrast(WHITE, rgb("#FF6B00"))).toBeLessThan(3);
  });
});

describe("readableOn", () => {
  it("leaves a colour alone when it already reads", () => {
    expect(readableOn(rgb("#FF7A1A"), DARK_SURFACE)).toEqual(rgb("#FF7A1A"));
  });

  it("lightens on dark and darkens on light, just enough", () => {
    const navy = rgb("#10206A");
    const onDark = readableOn(navy, DARK_SURFACE);
    expect(contrast(onDark, DARK_SURFACE)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(onDark, DARK_SURFACE)).toBeLessThan(5.2);
    const pale = rgb("#FFE9A8");
    const onLight = readableOn(pale, WHITE);
    expect(contrast(onLight, WHITE)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(onLight, WHITE)).toBeLessThan(5.2);
    // The hue survives the move.
    expect(rgbToOklch(onDark)[2]).toBeCloseTo(rgbToOklch(navy)[2], -1);
  });
});

describe("deriveAccent", () => {
  it("reproduces the shipped orange", () => {
    const dark = deriveAccent("#FF6B00", "dark");
    expect(dark["--accent"]).toBe("#FF6B00");
    expect(dark["--on-accent"]).toBe("#0D0D0F");
    expect(dark["--accent-muted"]).toBe("rgb(255 107 0 / 0.16)");
    expect(deriveAccent("#FF6B00", "light")["--accent-muted"]).toBe("rgb(255 107 0 / 0.14)");
    // Hover is lighter and pressed darker than the accent itself.
    const l = (hex: string) => rgbToOklch(rgb(hex))[0];
    expect(l(dark["--accent-hover"])).toBeGreaterThan(l("#FF6B00"));
    expect(l(dark["--accent-active"])).toBeLessThan(l("#FF6B00"));
  });

  it("keeps text readable for every swatch and for extreme colours, in both themes", () => {
    const colours = [
      ...ACCENT_SWATCHES.map((s) => s.hex),
      "#FFFFF0",
      "#FFF4B8",
      "#05051A",
      "#000000",
      "#FFFFFF",
      "#808080",
    ];
    for (const hex of colours) {
      for (const theme of ["dark", "light"] as const) {
        const t = deriveAccent(hex, theme);
        const surface = theme === "dark" ? DARK_SURFACE : WHITE;
        expect(
          contrast(rgb(t["--accent-text"]), surface),
          `${hex} text on ${theme}`,
        ).toBeGreaterThanOrEqual(4.5);
        expect(
          contrast(rgb(t["--on-accent"]), rgb(t["--accent"])),
          `${hex} on-accent`,
        ).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it("keeps the focus ring visible when the accent blends into the surface", () => {
    const ring = (hex: string, theme: "dark" | "light") =>
      contrast(
        rgb(deriveAccent(hex, theme)["--focus-ring"]),
        theme === "dark" ? DARK_SURFACE : WHITE,
      );
    for (const hex of ["#FFFFF0", "#05051A", "#FFF4B8", "#000000", "#FFFFFF", "#808080"]) {
      expect(ring(hex, "dark"), `${hex} dark`).toBeGreaterThanOrEqual(3);
      expect(ring(hex, "light"), `${hex} light`).toBeGreaterThanOrEqual(3);
    }
    // An accent that already stands out is its own ring.
    expect(deriveAccent("#3E8BFF", "dark")["--focus-ring"]).toBe("#3E8BFF");
    expect(deriveAccent("#05051A", "light")["--focus-ring"]).toBe("#05051A");
    expect(deriveAccent("#05051A", "dark")["--focus-ring"]).toBe(
      deriveAccent("#05051A", "dark")["--accent-text"],
    );
  });

  it("puts white on dark accents and near-black on light ones", () => {
    expect(deriveAccent("#05051A", "dark")["--on-accent"]).toBe("#FFFFFF");
    expect(deriveAccent("#FFF4B8", "dark")["--on-accent"]).toBe("#0D0D0F");
  });

  it("falls back to the default for a colour that is not one", () => {
    expect(deriveAccent("orange", "dark")).toEqual(deriveAccent("#FF6B00", "dark"));
  });
});
