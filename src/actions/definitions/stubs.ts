/**
 * Placeholders for actions whose behaviour arrives in a later phase (IMPLEMENTATION_PLAN).
 * Registering them now keeps the registry complete: menus, the keymap, and the hotkey editor
 * all see every id from day one. A placeholder is disabled and does nothing.
 */
import type { Action, ActionGroup } from "../types";

type Stub = [id: string, label: string, group: ActionGroup, phase: string];

const STUBS: Stub[] = [
  ["window.toggle_pip", "Picture-in-picture", "window", "Phase 11"],
  ["ui.open_settings", "Settings", "window", "Phase 10"],
  ["ui.show_shortcuts", "Keyboard shortcuts", "window", "Phase 10"],
  ["ui.command_palette", "Command palette", "window", "Phase 10"],
  // Context-scoped (§2 "Context-scoped actions")
  ["settings.focus_search", "Search settings", "context", "Phase 10"],
];

export const stubActions: Action[] = STUBS.map(([id, label, group, phase]) => ({
  id,
  label,
  group,
  comingIn: phase,
  run: () => undefined,
}));
