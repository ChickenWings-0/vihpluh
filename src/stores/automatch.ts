/**
 * Subtitle auto-match view state (DESIGN_SYSTEM §10.1): the last report and whether a run is
 * in progress. The matching itself is the core's (`sub_automatch`).
 */
import { create } from "zustand";
import type { MatchScope } from "@/ipc/bindings/MatchScope";
import type { SubMatchReport } from "@/ipc/bindings/SubMatchReport";
import { subAutomatch } from "@/ipc/commands";
import { reportError } from "./osd";
import { useQueueStore } from "./queue";

interface AutomatchStore {
  report: SubMatchReport | null;
  running: boolean;
  /** Previews (`apply = false`) or saves the matches. */
  run: (apply: boolean) => Promise<void>;
  dismiss: () => void;
}

/** A queue of several local files is matched as a whole; otherwise the current folder. */
export function matchScope(localQueueItems: number): MatchScope {
  return localQueueItems > 1 ? "queue" : "folder";
}

export const useAutomatchStore = create<AutomatchStore>((set, get) => ({
  report: null,
  running: false,
  run: async (apply) => {
    if (get().running) return;
    set({ running: true });
    try {
      const items = useQueueStore.getState().queue?.items ?? [];
      const scope = matchScope(items.filter((item) => !item.isUrl).length);
      set({ report: await subAutomatch(scope, apply) });
    } catch (error) {
      reportError(error);
    } finally {
      set({ running: false });
    }
  },
  dismiss: () => set({ report: null }),
}));
