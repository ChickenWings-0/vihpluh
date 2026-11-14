/**
 * Turns input events into binding strings (KEYBINDS_AND_CONFIG §3.1): modifiers in the fixed
 * order Ctrl, Alt, Shift, Meta, then a key named by its physical US-layout position
 * (`KeyboardEvent.code`), so bindings do not move when the OS layout changes.
 */

const CODE_NAMES: Record<string, string> = {
  Space: "Space",
  Enter: "Enter",
  Escape: "Escape",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  ContextMenu: "ContextMenu",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  IntlBackslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  NumpadAdd: "NumAdd",
  NumpadSubtract: "NumSubtract",
  NumpadMultiply: "NumMultiply",
  NumpadDivide: "NumDivide",
  NumpadEnter: "NumEnter",
  NumpadDecimal: "NumDecimal",
  MediaPlayPause: "MediaPlayPause",
  MediaStop: "MediaStop",
  MediaTrackNext: "MediaTrackNext",
  MediaTrackPrevious: "MediaTrackPrevious",
};

/** The key part of a binding for a `KeyboardEvent.code`, or null for modifiers/unknown keys. */
export function keyFromCode(code: string): string | null {
  const named = CODE_NAMES[code];
  if (named) return named;
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter?.[1]) return letter[1];
  const digit = /^Digit([0-9])$/.exec(code);
  if (digit?.[1]) return digit[1];
  const numpad = /^Numpad([0-9])$/.exec(code);
  if (numpad?.[1]) return `Num${numpad[1]}`;
  const fkey = /^F([1-9]|1[0-9]|2[0-4])$/.exec(code);
  if (fkey?.[1]) return `F${fkey[1]}`;
  return null;
}

export interface Modifiers {
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

export function withModifiers(key: string, m: Modifiers): string {
  const parts: string[] = [];
  if (m.ctrlKey) parts.push("Ctrl");
  if (m.altKey) parts.push("Alt");
  if (m.shiftKey) parts.push("Shift");
  if (m.metaKey) parts.push("Meta");
  parts.push(key);
  return parts.join("+");
}

/** `null` for a pure modifier press or a key vihpluh has no name for. */
export function bindingFromKeyboard(e: Pick<KeyboardEvent, "code"> & Modifiers): string | null {
  const key = keyFromCode(e.code);
  return key ? withModifiers(key, e) : null;
}

const BUTTONS: Record<number, string> = {
  0: "MouseLeft",
  1: "MouseMiddle",
  2: "MouseRight",
  3: "MouseBack",
  4: "MouseForward",
};

export function bindingFromMouse(
  e: Pick<MouseEvent, "button"> & Modifiers,
  double = false,
): string | null {
  const name = e.button === 0 && double ? "MouseLeftDouble" : BUTTONS[e.button];
  return name ? withModifiers(name, e) : null;
}

/**
 * Wheel direction as a binding. `invert` implements `controls.wheel_invert`. The dominant
 * axis wins; tiny deltas (trackpad noise) are ignored.
 */
export function bindingFromWheel(
  e: Pick<WheelEvent, "deltaX" | "deltaY"> & Modifiers,
  invert = false,
): string | null {
  const { deltaX, deltaY } = e;
  if (Math.abs(deltaX) < 1 && Math.abs(deltaY) < 1) return null;
  let name: string;
  if (Math.abs(deltaY) >= Math.abs(deltaX)) {
    name = deltaY < 0 !== invert ? "WheelUp" : "WheelDown";
  } else {
    name = deltaX < 0 !== invert ? "WheelLeft" : "WheelRight";
  }
  return withModifiers(name, e);
}

/**
 * Glyphs of the current keyboard layout for the keys bindings name by position (§3.1): on an
 * AZERTY layout the key bound as `Q` is shown as `A`. Filled from the Keyboard Layout Map API
 * when the webview has it; otherwise the US names are shown.
 */
let layoutGlyphs: ReadonlyMap<string, string> = new Map();

export function setLayoutGlyphs(codeToGlyph: Iterable<[code: string, glyph: string]>): void {
  const glyphs = new Map<string, string>();
  for (const [code, glyph] of codeToGlyph) {
    const key = keyFromCode(code);
    // Only printable single keys differ between layouts.
    if (key?.length !== 1 || glyph.trim().length !== 1) continue;
    const shown = glyph.toUpperCase();
    if (shown !== key) glyphs.set(key, shown);
  }
  layoutGlyphs = glyphs;
}

const PRETTY: Record<string, string> = {
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
  MouseLeft: "Click",
  MouseLeftDouble: "Double-click",
  MouseMiddle: "Middle-click",
  MouseRight: "Right-click",
  MouseBack: "Mouse back",
  MouseForward: "Mouse forward",
  WheelUp: "Wheel ↑",
  WheelDown: "Wheel ↓",
  WheelLeft: "Wheel ←",
  WheelRight: "Wheel →",
  Escape: "Esc",
};

/** How a binding is shown on a key cap: arrows and mouse names, and the layout's own glyphs. */
export function displayBinding(binding: string): string {
  // The key itself may be "+", so only the modifiers are split off.
  const parts = binding.split("+");
  return parts
    .map((part, i) =>
      i === parts.length - 1 ? (PRETTY[part] ?? layoutGlyphs.get(part) ?? part) : part,
    )
    .join("+");
}
