/**
 * Mirror of the core's `PlayerState` (ARCHITECTURE §4.2), fed by `player://patch`.
 * Components select narrowly — never subscribe to the whole state.
 */
import { create } from "zustand";
import type { PlayerState } from "@/ipc/bindings/PlayerState";
import type { PlayerPatch } from "@/ipc/events";

export type PatchResult = "applied" | "stale" | "gap" | "no-base";

interface PlayerStore {
  state: PlayerState | null;
  /** Replaces the state with a snapshot unless a newer patch already arrived. */
  setSnapshot: (snapshot: PlayerState) => void;
  /** Applies a patch. `gap` means one was missed and the caller must re-fetch a snapshot. */
  applyPatch: (patch: PlayerPatch) => PatchResult;
}

/** Pure patch application, exported for tests. */
export function mergePatch(
  state: PlayerState | null,
  patch: PlayerPatch,
): { state: PlayerState | null; result: PatchResult } {
  if (state === null) return { state, result: "no-base" };
  if (patch.rev <= state.rev) return { state, result: "stale" };
  if (patch.rev !== state.rev + 1) return { state, result: "gap" };
  return { state: { ...state, ...patch } as PlayerState, result: "applied" };
}

export const usePlayerStore = create<PlayerStore>((set, get) => ({
  state: null,
  setSnapshot: (snapshot) => {
    const current = get().state;
    if (current === null || snapshot.rev >= current.rev) set({ state: snapshot });
  },
  applyPatch: (patch) => {
    const { state, result } = mergePatch(get().state, patch);
    if (result === "applied") set({ state });
    return result;
  },
}));

/** Narrow selector helper: `usePlayer((s) => s.volume)`; undefined until the first snapshot. */
export function usePlayer<T>(select: (state: PlayerState) => T): T | undefined {
  return usePlayerStore((store) => (store.state ? select(store.state) : undefined));
}
