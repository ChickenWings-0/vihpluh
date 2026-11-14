/**
 * Placeholders for actions whose behaviour arrives in a later phase (IMPLEMENTATION_PLAN).
 * Registering them now keeps the registry complete: menus, the keymap, and the hotkey editor
 * all see every id from day one. A placeholder is disabled and does nothing.
 */
import type { Action, ActionGroup } from "../types";

type Stub = [id: string, label: string, group: ActionGroup, phase: string];

const STUBS: Stub[] = [["window.toggle_pip", "Picture-in-picture", "window", "Phase 11"]];

export const stubActions: Action[] = STUBS.map(([id, label, group, phase]) => ({
  id,
  label,
  group,
  comingIn: phase,
  run: () => undefined,
}));
