/**
 * Placeholders for actions whose behaviour arrives in a later phase (IMPLEMENTATION_PLAN).
 * Registering them now keeps the registry complete: menus, the keymap, and the hotkey editor
 * all see every id from day one. A placeholder is disabled and does nothing.
 */
import type { Action, ActionGroup } from "../types";

type Stub = [id: string, label: string, group: ActionGroup, phase: string];

const STUBS: Stub[] = [
  ["queue.save_as_playlist", "Save queue as playlist…", "queue", "Phase 9"],
  ["markers.next", "Next chapter or bookmark", "queue", "Phase 8"],
  ["markers.previous", "Previous chapter or bookmark", "queue", "Phase 8"],
  ["bookmarks.add", "Add bookmark here", "queue", "Phase 8"],
  ["bookmarks.toggle_panel", "Show bookmarks and chapters", "queue", "Phase 8"],
  ["subtitles.sync_previous_cue", "Show the previous line now", "subtitles", "Phase 7"],
  ["subtitles.sync_next_cue", "Show the next line now", "subtitles", "Phase 7"],
  ["subtitles.size_decrease", "Subtitle size −", "subtitles", "Phase 7"],
  ["subtitles.size_increase", "Subtitle size +", "subtitles", "Phase 7"],
  ["subtitles.position_up", "Move subtitles up", "subtitles", "Phase 7"],
  ["subtitles.position_down", "Move subtitles down", "subtitles", "Phase 7"],
  ["subtitles.add_file", "Add subtitle file…", "subtitles", "Phase 7"],
  ["subtitles.automatch", "Auto-match subtitles in folder", "subtitles", "Phase 7"],
  ["window.toggle_pip", "Picture-in-picture", "window", "Phase 11"],
  ["ui.sidebar_subtitles", "Subtitles panel", "window", "Phase 7"],
  ["ui.sidebar_bookmarks", "Bookmarks panel", "window", "Phase 8"],
  ["ui.toggle_library", "Library", "window", "Phase 9"],
  ["ui.open_settings", "Settings", "window", "Phase 10"],
  ["ui.show_shortcuts", "Keyboard shortcuts", "window", "Phase 10"],
  ["ui.command_palette", "Command palette", "window", "Phase 10"],
  ["app.open_url", "Open URL…", "app", "Phase 9"],
  ["presets.save_current", "Save current settings as preset…", "app", "Phase 8"],
  ["presets.update_current", "Update applied preset", "app", "Phase 8"],
  ["presets.apply", "Apply preset", "app", "Phase 8"],
  // Context-scoped (§2 "Context-scoped actions")
  ["library.play_selected", "Play now", "context", "Phase 9"],
  ["library.play_next_selected", "Play next", "context", "Phase 9"],
  ["library.enqueue_selected", "Add to queue", "context", "Phase 9"],
  ["library.focus_search", "Search", "context", "Phase 9"],
  ["library.go_up", "Up one level", "context", "Phase 9"],
  ["bookmarks.rename_selected", "Rename", "context", "Phase 8"],
  ["bookmarks.remove_selected", "Delete", "context", "Phase 8"],
  ["settings.focus_search", "Search settings", "context", "Phase 10"],
];

export const stubActions: Action[] = STUBS.map(([id, label, group, phase]) => ({
  id,
  label,
  group,
  comingIn: phase,
  run: () => undefined,
}));
