/** Window and interface actions (KEYBINDS_AND_CONFIG §2 "Window and interface"). */
import { Expand, Minimize2, Pin, Scan, Shrink } from "lucide-react";
import { windowMinimize, windowSetAlwaysOnTop, windowSetMode } from "@/ipc/commands";
import { useOsdStore } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";
import type { Action } from "../types";

const ui = () => useUiStore.getState();

export const PLAYER_ROOT_ID = "player-root";

/** Returns keyboard focus to the player root, so single-key shortcuts are live again. */
export function focusPlayerRoot(): void {
  document.getElementById(PLAYER_ROOT_ID)?.focus({ preventScroll: true });
}

async function setFullscreen(on: boolean) {
  await windowSetMode(on ? "fullscreen" : "normal");
  return { icon: on ? Expand : Shrink, label: on ? "Fullscreen" : "Windowed" };
}

export const windowActions: Action[] = [
  {
    id: "window.toggle_fullscreen",
    label: "Fullscreen",
    group: "window",
    icon: Expand,
    isOn: () => ui().window.mode === "fullscreen",
    run: () => setFullscreen(ui().window.mode !== "fullscreen"),
  },
  {
    id: "window.toggle_zen",
    label: "Zen mode",
    group: "window",
    icon: Scan,
    isOn: () => ui().zen,
    run: () => {
      const entering = !ui().zen;
      const firstTime = entering && !ui().zenHintShown;
      ui().setZen(entering);
      if (firstTime) {
        // Shown once per session, even though Zen hides the OSD for everything else.
        useOsdStore.getState().showFlash({ icon: Scan, label: "Zen", value: "Z to exit" }, 1000);
      }
      return undefined;
    },
  },
  {
    id: "window.toggle_always_on_top",
    label: "Always on top",
    group: "window",
    icon: Pin,
    isOn: () => ui().window.alwaysOnTop,
    run: async () => {
      const on = !ui().window.alwaysOnTop;
      await windowSetAlwaysOnTop(on);
      return { icon: Pin, label: on ? "Always on top" : "Not on top" };
    },
  },
  {
    id: "window.minimize",
    label: "Minimize",
    group: "window",
    icon: Minimize2,
    run: async () => {
      await windowMinimize();
      return undefined;
    },
  },
  {
    // The first applicable step runs, then stops (§2 "ui.escape chain"). Menus and popovers
    // close themselves on Escape before this runs; overlays join the chain in later phases.
    // It never closes the dock and never pauses.
    id: "ui.escape",
    label: "Back out",
    group: "window",
    run: async () => {
      const state = ui();
      if (state.contextMenu) {
        state.closeContextMenu();
        return undefined;
      }
      const active = document.activeElement;
      if (
        active instanceof HTMLElement &&
        active.id !== PLAYER_ROOT_ID &&
        active !== document.body
      ) {
        active.blur();
        focusPlayerRoot();
        return undefined;
      }
      if (state.zen) {
        state.setZen(false);
        return undefined;
      }
      if (state.window.mode === "fullscreen") return setFullscreen(false);
      return undefined;
    },
  },
  {
    id: "ui.context_menu",
    label: "Context menu",
    group: "window",
    run: () => {
      ui().openContextMenu();
      return undefined;
    },
  },
];
