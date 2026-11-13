/**
 * The action registry (KEYBINDS_AND_CONFIG §2): every user-triggerable behaviour has an id.
 * Buttons, menus, the context menu, shortcuts, and (later) the palette all dispatch here.
 */

import { displayBinding } from "@/keybinds/normalize";
import { useConfigStore } from "@/stores/config";
import { useOsdStore } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";
import type { Action, RunContext, RunResult } from "./types";

const actions = new Map<string, Action>();

export function register(defs: readonly Action[]): void {
  for (const def of defs) {
    if (actions.has(def.id)) throw new Error(`action ${def.id} registered twice`);
    actions.set(def.id, def);
  }
}

export function getAction(id: string): Action | undefined {
  return actions.get(id);
}

export function allActions(): Action[] {
  return [...actions.values()];
}

export function isEnabled(id: string): boolean {
  const action = actions.get(id);
  if (!action || action.comingIn) return false;
  return action.isEnabled ? action.isEnabled() : true;
}

/** Runs an action. Unknown or disabled actions do nothing. */
export async function dispatch(id: string, ctx: RunContext): Promise<void> {
  const action = actions.get(id);
  if (!action || !isEnabled(id)) return;
  if (ctx.repeat && !action.repeatable) return;
  let result: RunResult;
  try {
    result = await action.run(ctx);
  } catch (error) {
    console.error(`action ${id} failed`, error);
    const message =
      typeof error === "object" && error !== null && "message" in error
        ? String((error as { message: unknown }).message)
        : String(error);
    useOsdStore.getState().pushToast({ kind: "error", text: message });
    return;
  }
  showFeedback(action, ctx, result);
}

/**
 * Keyboard, wheel, and video-area mouse actions flash the OSD while the chrome is hidden
 * (DESIGN_SYSTEM §7.1, §8.9). With the shortcut badge on, key-triggered actions always flash
 * and show the key that fired them.
 */
function showFeedback(action: Action, ctx: RunContext, result: RunResult): void {
  if (ctx.trigger === "button" || ctx.trigger === "menu" || ctx.trigger === "palette") return;
  const config = useConfigStore.getState().config;
  const ui = useUiStore.getState();
  if (config && !config.interface.osd_enabled) return;
  if (ui.zen) return;
  const badgeOn = Boolean(config?.interface.osd_shortcut_badge) && ctx.trigger === "key";
  if (!badgeOn && (ui.bottomVisible || !result)) return;
  const feedback = result ?? { label: action.label, icon: action.icon };
  useOsdStore.getState().showFlash(
    {
      icon: feedback.icon ?? action.icon,
      label: feedback.label,
      value: feedback.value,
      bar: feedback.bar,
      badge: badgeOn && ctx.binding ? displayBinding(ctx.binding) : undefined,
    },
    config?.interface.osd_duration_ms ?? 1200,
  );
}

/** Test helper. */
export function clearRegistry(): void {
  actions.clear();
}
