/**
 * Binding resolution (KEYBINDS_AND_CONFIG §3.2): which action, if any, a binding triggers in
 * the current situation. Pure functions over the resolved keymap from the core.
 */
import type { CustomBinding } from "@/ipc/bindings/CustomBinding";
import type { ResolvedKeymap } from "@/ipc/bindings/ResolvedKeymap";

export type KeyContext = "global" | "queue" | "library" | "bookmarks" | "settings";

export interface Lookup {
  /** context → binding → action id */
  byContext: Map<string, Map<string, string>>;
  /** binding → custom binding (checked first) */
  custom: Map<string, CustomBinding>;
  /** action id → keys, for menus and tooltips (global context) */
  keysOf: Map<string, string[]>;
}

export function buildLookup(keymap: ResolvedKeymap): Lookup {
  const byContext = new Map<string, Map<string, string>>();
  for (const [context, actions] of Object.entries(keymap.contexts)) {
    const map = new Map<string, string>();
    for (const [action, keys] of Object.entries(actions)) {
      for (const key of keys) map.set(key, action);
    }
    byContext.set(context, map);
  }
  const custom = new Map<string, CustomBinding>();
  for (const binding of keymap.custom) {
    for (const key of binding.keys) custom.set(key, binding);
  }
  const keysOf = new Map<string, string[]>();
  for (const [context, actions] of Object.entries(keymap.contexts)) {
    for (const [action, keys] of Object.entries(actions)) {
      if (context === "global" || !keysOf.has(action)) keysOf.set(action, keys);
    }
  }
  return { byContext, custom, keysOf };
}

export type Match =
  | { kind: "action"; actionId: string }
  | { kind: "custom"; binding: CustomBinding };

export interface Situation {
  context: KeyContext;
  /** Focus is in a text field: only Ctrl/Alt bindings and Escape apply. */
  inTextInput: boolean;
  /**
   * A control has keyboard focus and owns these keys natively (Space, Enter, arrows,
   * Home/End): they are left to it.
   */
  focusedControlKeys: ReadonlySet<string>;
}

/** The keys a focused native control keeps for itself (§3.2 rule 3). */
export const CONTROL_KEYS: ReadonlySet<string> = new Set([
  "Space",
  "Enter",
  "Up",
  "Down",
  "Left",
  "Right",
  "Home",
  "End",
  "PageUp",
  "PageDown",
]);

export function resolveBinding(lookup: Lookup, binding: string, s: Situation): Match | null {
  if (s.inTextInput) {
    const hasCtrlOrAlt = binding.startsWith("Ctrl+") || binding.startsWith("Alt+");
    if (!hasCtrlOrAlt && binding !== "Escape") return null;
  }
  if (s.focusedControlKeys.has(binding)) return null;
  const custom = lookup.custom.get(binding);
  if (custom) return { kind: "custom", binding: custom };
  for (const context of s.context === "global" ? ["global"] : [s.context, "global"]) {
    const actionId = lookup.byContext.get(context)?.get(binding);
    if (actionId) return { kind: "action", actionId };
  }
  return null;
}
