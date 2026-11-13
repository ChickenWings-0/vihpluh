import { beforeAll, describe, expect, it } from "vitest";
import keymapText from "../../src-tauri/resources/keymaps/vihpluh.toml?raw";
import { cleanPastedSource } from "./definitions/app";
import { nextTrackId } from "./definitions/tracks";
import { allActions, getAction, isEnabled, registerActions } from "./index";

beforeAll(() => registerActions());

const PARAMETERISED = [
  "player.seek_relative",
  "player.seek_absolute",
  "player.set_speed",
  "player.set_volume",
  "audio.set_delay",
  "subtitles.set_delay",
  "audio.set_equalizer_preset",
  "video.set_aspect",
  "presets.apply",
  "player.set_sleep_timer",
  "mpv.command",
];

describe("action registry", () => {
  it("has every action the shipped keymap names, and nothing it doesn't", () => {
    const inKeymap = new Set(
      [...keymapText.matchAll(/^"([a-z0-9_.]+)"\s*=/gm)].map((m) => m[1] as string),
    );
    const registered = new Set(allActions().map((a) => a.id));
    const missing = [...inKeymap].filter((id) => !registered.has(id));
    const extra = [...registered].filter((id) => !inKeymap.has(id) && !PARAMETERISED.includes(id));
    expect(missing).toEqual([]);
    expect(extra).toEqual([]);
    for (const id of PARAMETERISED) expect(registered.has(id), id).toBe(true);
  });

  it("disables placeholders", () => {
    expect(getAction("queue.next")?.comingIn).toBe("Phase 4");
    expect(isEnabled("queue.next")).toBe(false);
    expect(isEnabled("app.quit")).toBe(true);
  });

  it("gives every action a label", () => {
    for (const action of allActions()) expect(action.label.length, action.id).toBeGreaterThan(0);
  });
});

describe("nextTrackId", () => {
  it("cycles through tracks, optionally via off", () => {
    expect(nextTrackId([1, 2, 3], 1, false)).toBe(2);
    expect(nextTrackId([1, 2, 3], 3, false)).toBe(1);
    expect(nextTrackId([1, 2], 2, true)).toBeNull();
    expect(nextTrackId([1, 2], null, true)).toBe(1);
    expect(nextTrackId([], null, true)).toBeNull();
  });
});

describe("cleanPastedSource", () => {
  it("takes the first non-empty line without quotes", () => {
    expect(cleanPastedSource('  "C:\\Videos\\a b.mkv"  ')).toBe("C:\\Videos\\a b.mkv");
    expect(cleanPastedSource("\n\nhttps://example.com/x.m3u8\nsecond")).toBe(
      "https://example.com/x.m3u8",
    );
    expect(cleanPastedSource("   \n ")).toBeNull();
  });
});
