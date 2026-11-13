/**
 * View state owned by the frontend (ARCHITECTURE §4.2): window mode mirror, Zen, what is
 * holding the chrome open, the context menu, and the drop hint. Nothing here is persisted
 * (panel state joins `session.json` in Phase 4).
 */
import { create } from "zustand";
import type { UiState } from "@/ipc/bindings/UiState";
import type { WindowState } from "@/ipc/bindings/WindowState";
import type { KeyContext } from "@/keybinds/engine";

export type DropHint = "replace" | "enqueue" | null;

/** The right dock hosts one panel at a time (DESIGN_SYSTEM §6.1). */
export type DockPanel = "queue" | "sidebar" | null;

/** Sidebar tabs available so far (Bookmarks arrives in Phase 8). */
export const SIDEBAR_TABS = ["subtitles", "audio", "video", "playback"] as const;
export type SidebarTab = (typeof SIDEBAR_TABS)[number];
const asTab = (tab: string): SidebarTab =>
  (SIDEBAR_TABS as readonly string[]).includes(tab) ? (tab as SidebarTab) : "subtitles";

/** Maps a drop point to a queue position when the pointer is over the queue panel. */
export type DropResolver = (x: number, y: number) => number | null;

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
  dock: DockPanel;
  sidebarTab: SidebarTab;
  /** Bumped to scroll the Playback tab to the stream inspector. */
  inspectorRequest: number;
  /** Live width while the dock edge is being dragged (CSS px); null otherwise. */
  dockDragWidth: number | null;
  dropResolver: DropResolver | null;
  /** Where a file drop over the queue would land (insertion line), or null. */
  dropIndex: number | null;

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
  /** Opens a panel in the dock (or closes it when it is already open). */
  toggleDock: (panel: Exclude<DockPanel, null>) => void;
  setDock: (panel: DockPanel) => void;
  /** Opens the sidebar on `tab`; with `toggle`, closes it when that tab is already showing. */
  showSidebarTab: (tab: SidebarTab, toggle?: boolean) => void;
  requestInspector: () => void;
  setDockDragWidth: (width: number | null) => void;
  setDropResolver: (resolver: DropResolver | null) => void;
  setDropIndex: (index: number | null) => void;
  /** Restores panels from the session (`session_ui_get`). */
  hydrate: (ui: UiState) => void;
}

/** Saves panel state to `session.json`; injected by the sync layer to keep IPC out of here. */
let persistUi: (patch: Partial<UiState>) => void = () => {};
export function setUiPersister(fn: (patch: Partial<UiState>) => void): void {
  persistUi = fn;
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
  dock: null,
  sidebarTab: "subtitles",
  inspectorRequest: 0,
  dockDragWidth: null,
  dropResolver: null,
  dropIndex: null,

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
  toggleDock: (panel) => get().setDock(get().dock === panel ? null : panel),
  setDock: (dock) => {
    if (get().dock === dock) return;
    set({ dock });
    persistUi({ queueOpen: dock === "queue", sidebarOpen: dock === "sidebar" });
  },
  showSidebarTab: (tab, toggle = false) => {
    const s = get();
    if (toggle && s.dock === "sidebar" && s.sidebarTab === tab) {
      s.setDock(null);
      return;
    }
    if (s.sidebarTab !== tab) {
      set({ sidebarTab: tab });
      persistUi({ sidebarTab: tab });
    }
    s.setDock("sidebar");
  },
  requestInspector: () => set({ inspectorRequest: get().inspectorRequest + 1 }),
  setDockDragWidth: (dockDragWidth) => set({ dockDragWidth }),
  setDropResolver: (dropResolver) => set({ dropResolver }),
  setDropIndex: (dropIndex) => {
    if (get().dropIndex !== dropIndex) set({ dropIndex });
  },
  hydrate: (ui) =>
    set({
      dock: ui.queueOpen ? "queue" : ui.sidebarOpen ? "sidebar" : null,
      sidebarTab: asTab(ui.sidebarTab),
    }),
}));

export const isHeld = (holds: Record<string, true>) => Object.keys(holds).length > 0;
