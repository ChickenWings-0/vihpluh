/**
 * Placeholders for actions whose behaviour arrives in a later phase (IMPLEMENTATION_PLAN).
 * Registering them now keeps the registry complete: menus, the keymap, and the hotkey editor
 * all see every id from day one. A placeholder is disabled and does nothing.
 */
import type { Action, ActionGroup } from "../types";

type Stub = [id: string, label: string, group: ActionGroup, phase: string];

const STUBS: Stub[] = [
  ["queue.save_as_playlist", "Save queue as playlist…", "queue", "Phase 9"],
  ["window.toggle_pip", "Picture-in-picture", "window", "Phase 11"],
  ["ui.toggle_library", "Library", "window", "Phase 9"],
  ["ui.open_settings", "Settings", "window", "Phase 10"],
  ["ui.show_shortcuts", "Keyboard shortcuts", "window", "Phase 10"],
  ["ui.command_palette", "Command palette", "window", "Phase 10"],
  ["app.open_url", "Open URL…", "app", "Phase 9"],
  // Context-scoped (§2 "Context-scoped actions")
  ["library.play_selected", "Play now", "context", "Phase 9"],
  ["library.play_next_selected", "Play next", "context", "Phase 9"],
  ["library.enqueue_selected", "Add to queue", "context", "Phase 9"],
  ["library.focus_search", "Search", "context", "Phase 9"],
  ["library.go_up", "Up one level", "context", "Phase 9"],
  ["settings.focus_search", "Search settings", "context", "Phase 10"],
];

export const stubActions: Action[] = STUBS.map(([id, label, group, phase]) => ({
  id,
  label,
  group,
  comingIn: phase,
  run: () => undefined,
}));
