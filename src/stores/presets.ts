/**
 * The list of presets (summaries), refreshed when `presets://changed` fires, and whether the
 * preset strip's "Save as…" popover has been asked for (the `presets.save_current` action).
 */
import { create } from "zustand";
import type { PresetSummary } from "@/ipc/bindings/PresetSummary";
import { presetList } from "@/ipc/commands";
import { onPresetsChanged } from "@/ipc/events";

interface PresetStore {
  presets: PresetSummary[];
  /** Bumped to open the "Save as…" popover. */
  saveRequest: number;
  setPresets: (presets: PresetSummary[]) => void;
  requestSave: () => void;
}

export const usePresetStore = create<PresetStore>((set, get) => ({
  presets: [],
  saveRequest: 0,
  setPresets: (presets) => set({ presets }),
  requestSave: () => set({ saveRequest: get().saveRequest + 1 }),
}));

export function startPresetSync(): () => void {
  const load = () =>
    void presetList()
      .then((list) => usePresetStore.getState().setPresets(list))
      .catch(() => undefined);
  load();
  return onPresetsChanged(load);
}
