import { describe, expect, it } from "vitest";
import { setAtPath } from "./config";

describe("setAtPath", () => {
  const config = { audio: { max_volume: 130, device: "auto" }, mpv: { options: { a: "1" } } };

  it("replaces a nested value without touching siblings", () => {
    const next = setAtPath(config, "audio.max_volume", 150);
    expect(next.audio.max_volume).toBe(150);
    expect(next.audio.device).toBe("auto");
    expect(config.audio.max_volume).toBe(130);
  });

  it("keeps whole keys under free-form tables and removes on null", () => {
    const added = setAtPath(config, "mpv.options.dither-depth", "auto");
    expect(added.mpv.options).toEqual({ a: "1", "dither-depth": "auto" });
    const removed = setAtPath(added, "mpv.options.a", null);
    expect(removed.mpv.options).toEqual({ "dither-depth": "auto" });
  });
});
