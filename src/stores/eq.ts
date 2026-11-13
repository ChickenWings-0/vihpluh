/**
 * Equalizer presets (`eq_presets_list`): built-ins plus the user's own. Loaded on first use
 * and after a save or delete. The live EQ itself is player state (`PlayerState.eq`).
 */
import { create } from "zustand";
import type { EqPreset } from "@/ipc/bindings/EqPreset";
import type { EqState } from "@/ipc/bindings/EqState";
import { eqPresetsList } from "@/ipc/commands";
import { reportError } from "./osd";

interface EqStore {
  presets: EqPreset[];
  loaded: boolean;
  load: () => Promise<EqPreset[]>;
}

export const useEqStore = create<EqStore>((set, get) => ({
  presets: [],
  loaded: false,
  load: async () => {
    try {
      const presets = await eqPresetsList();
      set({ presets, loaded: true });
      return presets;
    } catch (error) {
      reportError(error);
      return get().presets;
    }
  },
}));

/** The EQ state for a preset, keeping the current on/off switch. */
export function eqFromPreset(preset: EqPreset, enabled: boolean): EqState {
  return {
    enabled,
    mode: preset.mode,
    preset: preset.id,
    preampDb: preset.preampDb,
    bands: preset.bands.map((b) => ({ ...b })),
  };
}
