import { describe, expect, it } from "vitest";
import type { ResolvedKeymap } from "@/ipc/bindings/ResolvedKeymap";
import { buildLookup, CONTROL_KEYS, resolveBinding, type Situation } from "./engine";
import {
  bindingFromKeyboard,
  bindingFromMouse,
  bindingFromWheel,
  displayBinding,
  keyFromCode,
} from "./normalize";

const none = { ctrlKey: false, altKey: false, shiftKey: false, metaKey: false };

describe("normalize", () => {
  it("names physical keys", () => {
    expect(keyFromCode("KeyK")).toBe("K");
    expect(keyFromCode("Digit0")).toBe("0");
    expect(keyFromCode("ArrowLeft")).toBe("Left");
    expect(keyFromCode("Backslash")).toBe("\\");
    expect(keyFromCode("Numpad4")).toBe("Num4");
    expect(keyFromCode("F11")).toBe("F11");
    expect(keyFromCode("ShiftLeft")).toBeNull();
    expect(keyFromCode("F25")).toBeNull();
  });

  it("orders modifiers canonically", () => {
    expect(
      bindingFromKeyboard({
        code: "KeyP",
        ctrlKey: true,
        altKey: false,
        shiftKey: true,
        metaKey: false,
      }),
    ).toBe("Ctrl+Shift+P");
    expect(bindingFromKeyboard({ code: "Comma", ...none, ctrlKey: true })).toBe("Ctrl+,");
    expect(bindingFromKeyboard({ code: "ControlLeft", ...none, ctrlKey: true })).toBeNull();
  });

  it("names mouse buttons and the wheel", () => {
    expect(bindingFromMouse({ button: 0, ...none })).toBe("MouseLeft");
    expect(bindingFromMouse({ button: 0, ...none }, true)).toBe("MouseLeftDouble");
    expect(bindingFromMouse({ button: 4, ...none })).toBe("MouseForward");
    expect(bindingFromWheel({ deltaX: 0, deltaY: -100, ...none })).toBe("WheelUp");
    expect(bindingFromWheel({ deltaX: 0, deltaY: 100, ...none, shiftKey: true })).toBe(
      "Shift+WheelDown",
    );
    expect(bindingFromWheel({ deltaX: 0, deltaY: -100, ...none }, true)).toBe("WheelDown");
    expect(bindingFromWheel({ deltaX: 0.2, deltaY: 0.3, ...none })).toBeNull();
  });

  it("displays bindings readably", () => {
    expect(displayBinding("Shift+Left")).toBe("Shift+←");
    expect(displayBinding("MouseLeftDouble")).toBe("Double-click");
  });
});

const keymap: ResolvedKeymap = {
  base: "vihpluh",
  contexts: {
    global: {
      "player.toggle_pause": ["Space", "K"],
      "player.speed_reset": ["Backspace"],
      "ui.escape": ["Escape"],
      "app.open_file": ["Ctrl+O"],
      "player.seek_back_small": ["Left"],
    },
    queue: { "queue.remove_selected": ["Delete", "Backspace"] },
  },
  custom: [{ keys: ["F9"], action: "player.set_speed", args: { value: 1.75 }, label: null }],
  warnings: [],
};

const plain: Situation = { context: "global", inTextInput: false, focusedControlKeys: new Set() };

describe("resolveBinding", () => {
  const lookup = buildLookup(keymap);

  it("finds global actions and lists keys per action", () => {
    expect(resolveBinding(lookup, "K", plain)).toEqual({
      kind: "action",
      actionId: "player.toggle_pause",
    });
    expect(lookup.keysOf.get("player.toggle_pause")).toEqual(["Space", "K"]);
    expect(resolveBinding(lookup, "Q", plain)).toBeNull();
  });

  it("checks custom bindings first", () => {
    const match = resolveBinding(lookup, "F9", plain);
    expect(match?.kind).toBe("custom");
  });

  it("lets a context override global and fall through to it", () => {
    const queue: Situation = { ...plain, context: "queue" };
    expect(resolveBinding(lookup, "Backspace", queue)).toEqual({
      kind: "action",
      actionId: "queue.remove_selected",
    });
    expect(resolveBinding(lookup, "Backspace", plain)).toEqual({
      kind: "action",
      actionId: "player.speed_reset",
    });
    expect(resolveBinding(lookup, "K", queue)?.kind).toBe("action");
  });

  it("only lets Ctrl/Alt bindings and Escape through from text fields", () => {
    const typing: Situation = { ...plain, inTextInput: true };
    expect(resolveBinding(lookup, "K", typing)).toBeNull();
    expect(resolveBinding(lookup, "Space", typing)).toBeNull();
    expect(resolveBinding(lookup, "Ctrl+O", typing)?.kind).toBe("action");
    expect(resolveBinding(lookup, "Escape", typing)?.kind).toBe("action");
  });

  it("leaves native keys to a keyboard-focused control", () => {
    const focused: Situation = { ...plain, focusedControlKeys: CONTROL_KEYS };
    expect(resolveBinding(lookup, "Space", focused)).toBeNull();
    expect(resolveBinding(lookup, "Left", focused)).toBeNull();
    expect(resolveBinding(lookup, "K", focused)?.kind).toBe("action");
  });
});
