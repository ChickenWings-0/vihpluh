/**
 * View state owned by the frontend (ARCHITECTURE §4.2): window mode mirror, Zen, what is
 * holding the chrome open, the context menu, and the drop hint. Nothing here is persisted
 * (panel state joins `session.json` in Phase 4).
 */
import { create } from "zustand";
import type { WindowState } from "@/ipc/bindings/WindowState";
import type { KeyContext } from "@/keybinds/engine";

export type DropHint = "replace" | "enqueue" | null;

interface UiStore {
  window: WindowState;
  zen: boolean;
  /** Whether the "Zen · Z to exit" hint was already shown this session. */
  zenHintShown: boolean;
  /** Reasons the chrome must stay visible (pointer over a bar, menu open, slider drag…). */
  holds: Record<string, true>;
  /** What the bars currently show (set by the auto-hide controller). */
  bottomVisible: boolean;
  topVisible: boolean;
  cursorHidden: boolean;
  /** One-shot request to flash the bottom bar (player.show_progress). */
  revealRequest: number;
  contextMenu: { x: number; y: number } | null;
  /** Last pointer position over the window, for keyboard-opened menus. */
  pointer: { x: number; y: number };
  keyContext: KeyContext;
  dropHint: DropHint;

  setWindow: (window: WindowState) => void;
  setZen: (zen: boolean) => void;
  hold: (reason: string, on: boolean) => void;
  setVisibility: (v: { bottom: boolean; top: boolean; cursorHidden: boolean }) => void;
  requestReveal: () => void;
  openContextMenu: (at?: { x: number; y: number }) => void;
  closeContextMenu: () => void;
  setPointer: (x: number, y: number) => void;
  setKeyContext: (context: KeyContext) => void;
  setDropHint: (hint: DropHint) => void;
}

export const useUiStore = create<UiStore>((set, get) => ({
  window: { mode: "normal", maximized: false, minimized: false, focused: true, alwaysOnTop: false },
  zen: false,
  zenHintShown: false,
  holds: {},
  bottomVisible: true,
  topVisible: true,
  cursorHidden: false,
  revealRequest: 0,
  contextMenu: null,
  pointer: { x: 0, y: 0 },
  keyContext: "global",
  dropHint: null,

  setWindow: (window) => set({ window }),
  setZen: (zen) => set({ zen, zenHintShown: get().zenHintShown || zen }),
  hold: (reason, on) => {
    const holds = get().holds;
    if (Boolean(holds[reason]) === on) return;
    const next = { ...holds };
    if (on) next[reason] = true;
    else delete next[reason];
    set({ holds: next });
  },
  setVisibility: ({ bottom, top, cursorHidden }) => {
    const s = get();
    if (s.bottomVisible !== bottom || s.topVisible !== top || s.cursorHidden !== cursorHidden) {
      set({ bottomVisible: bottom, topVisible: top, cursorHidden });
    }
  },
  requestReveal: () => set({ revealRequest: get().revealRequest + 1 }),
  openContextMenu: (at) => set({ contextMenu: at ?? get().pointer }),
  closeContextMenu: () => set({ contextMenu: null }),
  setPointer: (x, y) => set({ pointer: { x, y } }),
  setKeyContext: (keyContext) => set({ keyContext }),
  setDropHint: (dropHint) => set({ dropHint }),
}));

export const isHeld = (holds: Record<string, true>) => Object.keys(holds).length > 0;
