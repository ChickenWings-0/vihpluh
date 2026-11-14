import { describe, expect, it } from "vitest";
import { fuzzyFilter, fuzzyScore } from "./fuzzy";

const ACTIONS = [
  "Play / Pause",
  "Stream info",
  "Fullscreen",
  "Zen mode",
  "Next subtitle track",
  "Subtitle delay +",
  "Settings",
  "Show sidebar",
];

describe("fuzzy", () => {
  it("needs the characters in order", () => {
    expect(fuzzyScore("fs", "Fullscreen")).not.toBeNull();
    expect(fuzzyScore("sf", "Fullscreen")).toBeNull();
    expect(fuzzyScore("xyz", "Fullscreen")).toBeNull();
    expect(fuzzyScore("", "anything")).toBe(0);
  });

  it("ranks word starts and runs first", () => {
    expect(fuzzyFilter("set", ACTIONS, (a) => a)[0]).toBe("Settings");
    expect(fuzzyFilter("sub del", ACTIONS, (a) => a)[0]).toBe("Subtitle delay +");
    expect(fuzzyFilter("full", ACTIONS, (a) => a)).toEqual(["Fullscreen"]);
    expect(fuzzyFilter("ss", ACTIONS, (a) => a)[0]).toBe("Show sidebar");
  });

  it("ignores case and spaces, and keeps the order for an empty query", () => {
    expect(fuzzyFilter("ZEN", ACTIONS, (a) => a)).toEqual(["Zen mode"]);
    expect(fuzzyFilter("  ", ACTIONS, (a) => a)).toEqual(ACTIONS);
    expect(fuzzyFilter("qqq", ACTIONS, (a) => a)).toEqual([]);
  });
});
