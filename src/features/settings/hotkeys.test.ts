import { describe, expect, it } from "vitest";
import type { ResolvedKeymap } from "@/ipc/bindings/ResolvedKeymap";
import {
  type ActionInfo,
  buildGroups,
  customForKey,
  customLabel,
  filterGroups,
  findConflict,
  groupsForKey,
  RESERVED,
  resolveConflict,
  withKey,
  withoutKey,
} from "./hotkeys";

const ACTIONS: ActionInfo[] = [
  { id: "player.toggle_pause", label: "Play / Pause", group: "playback" },
  { id: "player.seek_forward_small", label: "Seek forward", group: "playback" },
  { id: "bookmarks.add", label: "Add bookmark here", group: "queue" },
  { id: "window.toggle_zen", label: "Zen mode", group: "window" },
  { id: "app.quit", label: "Quit", group: "app" },
  { id: "queue.remove_selected", label: "Remove", group: "context" },
  { id: "mpv.command", label: "Raw mpv command", group: "app" },
];

const keymap: ResolvedKeymap = {
  base: "vihpluh",
  contexts: {
    global: {
      "player.toggle_pause": ["Space", "K"],
      "player.seek_forward_small": [";"],
      "bookmarks.add": ["Ctrl+B"],
      "window.toggle_zen": ["Z", "MouseMiddle"],
      "app.quit": ["Ctrl+Q"],
    },
    queue: { "queue.remove_selected": ["Delete"] },
  },
  defaults: {
    global: {
      "player.toggle_pause": ["Space", "K"],
      "player.seek_forward_small": ["Right"],
      "bookmarks.add": ["Ctrl+B"],
      "window.toggle_zen": ["Z", "MouseMiddle"],
      "app.quit": ["Ctrl+Q"],
    },
    queue: { "queue.remove_selected": ["Delete"] },
  },
  custom: [
    {
      keys: ["Ctrl+D"],
      action: "mpv.command",
      args: { command: ["cycle", "deinterlace"] },
      label: null,
      index: 0,
    },
    {
      keys: ["F9"],
      action: "player.set_speed",
      args: { value: 2 },
      label: "Double speed",
      index: 2,
    },
  ],
  warnings: [],
};

describe("hotkey rows", () => {
  const groups = buildGroups(keymap, ACTIONS, ["mpv.command"]);

  it("groups actions as the reference does, with panel groups last", () => {
    expect(groups.map((g) => g.label)).toEqual([
      "Playback",
      "Queue, chapters, bookmarks",
      "Window and interface",
      "Queue",
    ]);
    expect(groups[2]?.rows.map((r) => r.actionId)).toEqual(["window.toggle_zen", "app.quit"]);
    expect(groups[3]?.context).toBe("queue");
    expect(groups[3]?.rows[0]?.context).toBe("queue");
  });

  it("leaves out actions that only take arguments", () => {
    const ids = groups.flatMap((g) => g.rows.map((r) => r.actionId));
    expect(ids).not.toContain("mpv.command");
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("lists every action even when the base keymap is empty", () => {
    const empty: ResolvedKeymap = { ...keymap, base: "none", contexts: {}, defaults: {} };
    const rows = buildGroups(empty, ACTIONS, ["mpv.command"]).flatMap((g) => g.rows);
    expect(rows).toHaveLength(6);
    expect(rows.every((r) => r.keys.length === 0 && !r.modified)).toBe(true);
  });

  it("marks the rows that differ from the base keymap", () => {
    const rows = groups.flatMap((g) => g.rows);
    expect(rows.filter((r) => r.modified).map((r) => r.actionId)).toEqual([
      "player.seek_forward_small",
    ]);
    expect(rows.find((r) => r.modified)?.defaults).toEqual(["Right"]);
  });

  it("filters by label or action id", () => {
    const ids = (query: string) =>
      filterGroups(groups, query).flatMap((g) => g.rows.map((r) => r.actionId));
    expect(ids("pause")).toEqual(["player.toggle_pause"]);
    expect(ids("window.")).toEqual(["window.toggle_zen"]);
    expect(ids("seek FORWARD")).toEqual(["player.seek_forward_small"]);
    expect(ids("")).toHaveLength(6);
    expect(ids("nothing like this")).toEqual([]);
  });

  it("finds what a key triggers", () => {
    expect(groupsForKey(groups, "K").flatMap((g) => g.rows.map((r) => r.actionId))).toEqual([
      "player.toggle_pause",
    ]);
    expect(groupsForKey(groups, "F12")).toEqual([]);
    expect(customForKey(keymap.custom, "F9").map((c) => c.index)).toEqual([2]);
  });
});

describe("capturing a key", () => {
  it("reports the action that already has it, in the same context only", () => {
    expect(findConflict(keymap, "global", "player.toggle_pause", "Ctrl+B")).toEqual({
      key: "Ctrl+B",
      actionId: "bookmarks.add",
    });
    expect(findConflict(keymap, "global", "bookmarks.add", "Ctrl+B")).toBeNull();
    expect(findConflict(keymap, "queue", "queue.remove_selected", "Ctrl+B")).toBeNull();
    expect(findConflict(keymap, "global", "app.quit", "F12")).toBeNull();
  });

  it("adds a key, or puts it in place of the one being edited", () => {
    expect(withKey(["Space", "K"], "P")).toEqual(["Space", "K", "P"]);
    expect(withKey(["Space", "K"], "K")).toEqual(["Space", "K"]);
    expect(withKey(["Space", "K"], "P", "Space")).toEqual(["P", "K"]);
    expect(withKey(["Space", "K"], "K", "Space")).toEqual(["K"]);
    expect(withoutKey(["Space", "K"], "K")).toEqual(["Space"]);
  });

  it("replace moves the key; keep both off unbinds it everywhere", () => {
    const conflict = { key: "Ctrl+B", actionId: "bookmarks.add" };
    const target = { context: "global", actionId: "player.toggle_pause" };
    expect(resolveConflict(keymap, target, conflict, "replace")).toEqual([
      { context: "global", actionId: "bookmarks.add", keys: [] },
      { context: "global", actionId: "player.toggle_pause", keys: ["Space", "K", "Ctrl+B"] },
    ]);
    expect(resolveConflict(keymap, target, conflict, "neither")).toEqual([
      { context: "global", actionId: "bookmarks.add", keys: [] },
    ]);
    // Editing an existing cap: replace swaps it; "keep both off" leaves this action alone.
    expect(
      resolveConflict(keymap, { ...target, replacing: "K" }, conflict, "replace")[1]?.keys,
    ).toEqual(["Space", "Ctrl+B"]);
    expect(
      resolveConflict(keymap, { ...target, replacing: "K" }, conflict, "neither"),
    ).toHaveLength(1);
  });

  it("refuses the focus traversal keys with a reason", () => {
    expect(RESERVED.Tab).toMatch(/cannot be a shortcut/);
    expect(RESERVED["Shift+Tab"]).toBeDefined();
    expect(RESERVED.Space).toBeUndefined();
  });
});

describe("custom bindings", () => {
  it("are named by their label, or by action and arguments", () => {
    expect(keymap.custom.map(customLabel)).toEqual([
      "mpv.command (command cycle deinterlace)",
      "Double speed",
    ]);
  });
});
