/** The resolved keymap from the core and its lookup tables (KEYBINDS_AND_CONFIG §3). */
import { create } from "zustand";
import type { ResolvedKeymap } from "@/ipc/bindings/ResolvedKeymap";
import { buildLookup, type Lookup } from "@/keybinds/engine";

interface KeymapStore {
  keymap: ResolvedKeymap | null;
  lookup: Lookup;
  setKeymap: (keymap: ResolvedKeymap) => void;
}

const empty: Lookup = { byContext: new Map(), custom: new Map(), keysOf: new Map() };

export const useKeymapStore = create<KeymapStore>((set) => ({
  keymap: null,
  lookup: empty,
  setKeymap: (keymap) => set({ keymap, lookup: buildLookup(keymap) }),
}));

/** The first key bound to an action (for tooltips and menus), or undefined. */
export function useShortcut(actionId: string): string | undefined {
  return useKeymapStore((s) => s.lookup.keysOf.get(actionId)?.[0]);
}
