/**
 * The hotkey editor's model (KEYBINDS_AND_CONFIG §4): the rows it lists, what a captured key
 * collides with, and what each resolution writes. Pure functions over the resolved keymap;
 * the component only renders them and calls `keybinds_set`.
 */
import type { ActionGroup } from "@/actions/types";
import type { CustomBinding } from "@/ipc/bindings/CustomBinding";
import type { ResolvedKeymap } from "@/ipc/bindings/ResolvedKeymap";

/** Groups in the order of §2. `app` actions are listed with the window ones, as there. */
export const GROUPS: { id: string; label: string; groups: ActionGroup[] }[] = [
  { id: "playback", label: "Playback", groups: ["playback"] },
  { id: "queue", label: "Queue, chapters, bookmarks", groups: ["queue"] },
  { id: "audio", label: "Audio", groups: ["audio"] },
  { id: "subtitles", label: "Subtitles", groups: ["subtitles"] },
  { id: "video", label: "Video", groups: ["video"] },
  { id: "window", label: "Window and interface", groups: ["window", "app"] },
];

export const CONTEXT_LABELS: Record<string, string> = {
  queue: "Queue",
  library: "Library",
  bookmarks: "Bookmarks",
  settings: "Settings",
};

/** Focus traversal keys cannot be bound (§3.1). */
export const RESERVED: Record<string, string> = {
  Tab: "Tab moves between controls and cannot be a shortcut.",
  "Shift+Tab": "Shift+Tab moves between controls and cannot be a shortcut.",
};

export interface ActionInfo {
  id: string;
  label: string;
  group: ActionGroup;
}

export interface HotkeyRow {
  context: string;
  actionId: string;
  label: string;
  keys: string[];
  defaults: string[];
  modified: boolean;
}

export interface HotkeyGroup {
  id: string;
  label: string;
  /** Set for the panel-scoped groups: shown as a chip on their rows. */
  context?: string;
  rows: HotkeyRow[];
}

const sameKeys = (a: string[], b: string[]) =>
  a.length === b.length && a.every((k, i) => k === b[i]);

function row(keymap: ResolvedKeymap, context: string, action: ActionInfo): HotkeyRow {
  const keys = keymap.contexts[context]?.[action.id] ?? [];
  const defaults = keymap.defaults[context]?.[action.id] ?? [];
  return {
    context,
    actionId: action.id,
    label: action.label,
    keys,
    defaults,
    modified: !sameKeys(keys, defaults),
  };
}

/**
 * Every bindable action as rows: the global ones grouped as in §2, then one group per panel
 * context (a panel action's id starts with its panel). The list comes from the action
 * registry, not the keymap, so the empty base keymap still shows everything. Actions that
 * take arguments (`skip`) are bound with `[[custom]]` and are not listed.
 */
export function buildGroups(
  keymap: ResolvedKeymap,
  actions: readonly ActionInfo[],
  skip: readonly string[] = [],
): HotkeyGroup[] {
  const bindable = actions.filter((a) => !skip.includes(a.id));
  const globalGroups = GROUPS.map((group) => ({
    id: group.id,
    label: group.label,
    rows: bindable
      .filter((a) => group.groups.includes(a.group))
      .map((a) => row(keymap, "global", a)),
  }));
  const contextGroups = Object.entries(CONTEXT_LABELS).map(([context, label]) => ({
    id: `context-${context}`,
    label,
    context,
    rows: bindable
      .filter((a) => a.group === "context" && a.id.startsWith(`${context}.`))
      .map((a) => row(keymap, context, a)),
  }));
  return [...globalGroups, ...contextGroups].filter((group) => group.rows.length > 0);
}

/** Rows whose label or action id contains every word of the query. */
export function filterGroups(groups: HotkeyGroup[], query: string): HotkeyGroup[] {
  const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return groups;
  return groups
    .map((group) => ({
      ...group,
      rows: group.rows.filter((r) => {
        const text = `${r.label} ${r.actionId}`.toLowerCase();
        return tokens.every((token) => text.includes(token));
      }),
    }))
    .filter((group) => group.rows.length > 0);
}

/** Find by key: only the rows that key triggers. */
export function groupsForKey(groups: HotkeyGroup[], key: string): HotkeyGroup[] {
  return groups
    .map((group) => ({ ...group, rows: group.rows.filter((r) => r.keys.includes(key)) }))
    .filter((group) => group.rows.length > 0);
}

export function customForKey(custom: readonly CustomBinding[], key: string): CustomBinding[] {
  return custom.filter((binding) => binding.keys.includes(key));
}

export interface Conflict {
  key: string;
  /** The action that already uses the key in the same context. */
  actionId: string;
}

/** The other action in `context` that `key` is bound to, if any. */
export function findConflict(
  keymap: ResolvedKeymap,
  context: string,
  actionId: string,
  key: string,
): Conflict | null {
  for (const [other, keys] of Object.entries(keymap.contexts[context] ?? {})) {
    if (other !== actionId && keys.includes(key)) return { key, actionId: other };
  }
  return null;
}

/** An action's keys after capturing `key`: added, or put in place of `replacing`. */
export function withKey(keys: string[], key: string, replacing?: string): string[] {
  if (replacing !== undefined) {
    const next = keys.map((k) => (k === replacing ? key : k));
    return next.filter((k, i) => next.indexOf(k) === i);
  }
  return keys.includes(key) ? keys : [...keys, key];
}

export const withoutKey = (keys: string[], key: string) => keys.filter((k) => k !== key);

export interface Write {
  context: string;
  actionId: string;
  keys: string[];
}

/**
 * What to write for a captured key that collides. "Replace" moves the key to this action;
 * "keep both off" leaves the key bound to neither (this action's own keys stay as they were).
 */
export function resolveConflict(
  keymap: ResolvedKeymap,
  target: { context: string; actionId: string; replacing?: string },
  conflict: Conflict,
  choice: "replace" | "neither",
): Write[] {
  const { context, actionId, replacing } = target;
  const otherKeys = keymap.contexts[context]?.[conflict.actionId] ?? [];
  const ownKeys = keymap.contexts[context]?.[actionId] ?? [];
  const writes: Write[] = [
    { context, actionId: conflict.actionId, keys: withoutKey(otherKeys, conflict.key) },
  ];
  if (choice === "replace") {
    writes.push({ context, actionId, keys: withKey(ownKeys, conflict.key, replacing) });
  }
  return writes;
}

/** A readable name for a `[[custom]]` binding without a label. */
export function customLabel(binding: CustomBinding): string {
  if (binding.label) return binding.label;
  const args = Object.entries(binding.args)
    .map(([name, value]) => `${name} ${Array.isArray(value) ? value.join(" ") : String(value)}`)
    .join(", ");
  return args ? `${binding.action} (${args})` : binding.action;
}
