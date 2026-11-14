import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { Capabilities } from "@/ipc/bindings/Capabilities";
import { isHex } from "@/lib/color";
import { CATEGORIES, isAvailable, SETTING_LIST, SETTINGS, sameValue } from "./schema";
import { highlight, searchSettings } from "./search";

/** The shipped defaults: `section.key` → value. Free-form and file-only tables are skipped. */
function shippedDefaults(): Map<string, unknown> {
  const text = readFileSync("src-tauri/resources/default.toml", "utf8");
  const values = new Map<string, unknown>();
  let section = "";
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (line === "" || line.startsWith("#")) continue;
    const header = /^\[(.+)\]$/.exec(line);
    if (header?.[1]) {
      section = header[1];
      continue;
    }
    const pair = /^([a-z0-9_]+)\s*=\s*(.+)$/.exec(line);
    if (!pair?.[1] || !pair[2] || section === "") continue;
    if (section === "mpv" || section.startsWith("mpv.") || section === "env") continue;
    // The defaults use only values TOML and JSON write the same way.
    values.set(`${section}.${pair[1]}`, JSON.parse(pair[2]));
  }
  return values;
}

/** Label and tooltip per key, from the tables of KEYBINDS_AND_CONFIG §5. */
function documented(): Map<string, { label: string; tooltip: string; restart: boolean }> {
  const whole = readFileSync("docs/KEYBINDS_AND_CONFIG.md", "utf8");
  // Only the config reference: the action tables of §2 use dotted ids too.
  const text = whole.slice(whole.indexOf("\n## 5. "), whole.indexOf("\n## 6. "));
  const rows = new Map<string, { label: string; tooltip: string; restart: boolean }>();
  for (const line of text.split("\n")) {
    const row = /^\| `([a-z]+\.[a-z0-9_]+)`( ⟳)? \|/.exec(line);
    if (!row?.[1]) continue;
    // Cells may contain `\|` inside enum types.
    const cells = line
      .replace(/\\\|/g, "¦")
      .split("|")
      .map((cell) => cell.replace(/¦/g, "|").trim());
    const label = cells[4] ?? "";
    // A trailing *(…)* is a note for readers of the document, not tooltip text.
    const tooltip = (cells[5] ?? "").replace(/\s*\*\(.*\)\*$/, "");
    rows.set(row[1], { label, tooltip, restart: row[2] !== undefined });
  }
  return rows;
}

const defaults = shippedDefaults();

describe("settings schema", () => {
  it("has exactly the keys of the shipped config", () => {
    expect(Object.keys(SETTINGS).sort()).toEqual([...defaults.keys()].sort());
    expect(SETTING_LIST.length).toBeGreaterThan(100);
  });

  it("puts every setting in one of the ten categories, in order", () => {
    expect(CATEGORIES.map((c) => c.label)).toEqual([
      "General",
      "Interface",
      "Controls",
      "Playback",
      "Video",
      "Audio",
      "Subtitles",
      "Library",
      "Network",
      "Performance",
    ]);
    const ids = CATEGORIES.map((c) => c.id as string);
    for (const setting of SETTING_LIST) expect(ids, setting.path).toContain(setting.category);
    // Pages list their settings together: categories never interleave.
    const order = SETTING_LIST.map((s) => ids.indexOf(s.category));
    expect(order).toEqual([...order].sort((a, b) => a - b));
  });

  it("gives every setting a label and a tooltip", () => {
    for (const setting of SETTING_LIST) {
      expect(setting.label.trim(), setting.path).not.toBe("");
      expect(setting.tooltip.trim(), setting.path).not.toBe("");
      expect(setting.tooltip.split(/[.;] /).length, `${setting.path} tooltip`).toBeLessThanOrEqual(
        4,
      );
    }
    const labels = SETTING_LIST.map((s) => `${s.category}/${s.label}`);
    expect(new Set(labels).size).toBe(labels.length);
  });

  it("matches the labels, tooltips, and restart marks of the reference document", () => {
    const doc = documented();
    for (const setting of SETTING_LIST) {
      const row = doc.get(setting.path);
      expect(row, `${setting.path} is documented`).toBeDefined();
      expect(setting.label, setting.path).toBe(row?.label);
      expect(setting.tooltip, setting.path).toBe(row?.tooltip);
      expect(Boolean(setting.restart), `${setting.path} restart`).toBe(row?.restart);
    }
    expect([...doc.keys()].sort()).toEqual(Object.keys(SETTINGS).sort());
  });

  it("gives each default a control that can show it", () => {
    for (const setting of SETTING_LIST) {
      const value = defaults.get(setting.path);
      const control = setting.control;
      const where = `${setting.path} (${control.kind})`;
      switch (control.kind) {
        case "toggle":
          expect(typeof value, where).toBe("boolean");
          break;
        case "choice":
          expect(
            control.options.map((o) => o.value),
            where,
          ).toContain(value);
          expect(control.options.length, where).toBeGreaterThanOrEqual(2);
          break;
        case "number":
          expect(typeof value, where).toBe("number");
          expect(value as number, where).toBeGreaterThanOrEqual(control.min);
          expect(value as number, where).toBeLessThanOrEqual(control.max);
          break;
        case "keyword":
          expect(
            control.options.map((o) => o.value),
            where,
          ).toContain(value);
          break;
        case "list":
          expect(Array.isArray(value), where).toBe(true);
          break;
        case "color":
          expect(isHex(String(value)), where).toBe(true);
          break;
        default:
          expect(typeof value, where).toBe("string");
      }
    }
  });

  it("marks exactly the documented settings as needing a restart", () => {
    expect(SETTING_LIST.filter((s) => s.restart).map((s) => s.path)).toEqual([
      "general.single_instance",
      "general.media_keys",
      "interface.window_decorations",
      "video.gpu_api",
      "video.linux_force_xwayland",
      "performance.webkit_dmabuf",
    ]);
  });

  it("hides what the platform cannot do", () => {
    const windows = {
      session: "windows",
      hdrPassthrough: true,
      windowPositioning: true,
    } as Capabilities;
    const wayland = {
      session: "wayland",
      hdrPassthrough: false,
      windowPositioning: false,
    } as Capabilities;
    const visible = (caps: Capabilities) =>
      new Set(SETTING_LIST.filter((s) => isAvailable(s, caps)).map((s) => s.path));
    expect(visible(windows).has("video.gpu_api")).toBe(true);
    expect(visible(windows).has("video.linux_force_xwayland")).toBe(false);
    expect(visible(windows).has("performance.webkit_dmabuf")).toBe(false);
    expect(visible(wayland).has("video.gpu_api")).toBe(false);
    expect(visible(wayland).has("video.hdr_passthrough")).toBe(false);
    expect(visible(wayland).has("interface.pip_corner")).toBe(false);
    expect(visible(wayland).has("video.linux_force_xwayland")).toBe(true);
    // Before capabilities are known nothing is hidden.
    expect(SETTING_LIST.every((s) => isAvailable(s, undefined))).toBe(true);
  });

  it("compares values the way the core does", () => {
    expect(sameValue(42, 42.0)).toBe(true);
    expect(sameValue(0.1 + 0.2, 0.3)).toBe(true);
    expect(sameValue(["eng", "en"], ["eng", "en"])).toBe(true);
    expect(sameValue(["eng"], ["eng", "en"])).toBe(false);
    expect(sameValue("auto", 203)).toBe(false);
  });
});

describe("settings search", () => {
  const paths = (query: string) => searchSettings(query, SETTING_LIST).map((s) => s.path);

  it("finds settings by what people call them", () => {
    expect(paths("subtitle size").slice(0, 3)).toEqual(
      expect.arrayContaining(["subtitles.font_size", "subtitles.scale"]),
    );
    expect(paths("hdr")).toEqual(
      expect.arrayContaining(["video.tone_mapping", "video.hdr_passthrough", "video.target_peak"]),
    );
    expect(paths("wayland")).toEqual(
      expect.arrayContaining(["video.linux_force_xwayland", "interface.pip_corner"]),
    );
    expect(paths("dark mode")[0]).toBe("interface.theme");
  });

  it("finds exactly one row for a pasted key", () => {
    expect(paths("performance.demuxer_cache_mb")).toEqual(["performance.demuxer_cache_mb"]);
    expect(paths("subtitles.font_size")[0]).toBe("subtitles.font_size");
  });

  it("needs every word and ignores case", () => {
    expect(paths("HDR Passthrough")[0]).toBe("video.hdr_passthrough");
    expect(paths("hdr banana")).toEqual([]);
    expect(paths("   ")).toEqual([]);
  });

  it("marks the matched text", () => {
    expect(highlight("HDR tone mapping", "hdr map")).toEqual([
      { text: "HDR", match: true },
      { text: " tone ", match: false },
      { text: "map", match: true },
      { text: "ping", match: false },
    ]);
    expect(highlight("Theme", "")).toEqual([{ text: "Theme", match: false }]);
  });
});
