/** Read-only access to current state for action implementations. */
import type { Config } from "@/ipc/bindings/Config";
import type { PlayerState } from "@/ipc/bindings/PlayerState";
import { useConfigStore } from "@/stores/config";
import { usePlayerStore } from "@/stores/player";

export const player = (): PlayerState | null => usePlayerStore.getState().state;
export const config = (): Config | null => useConfigStore.getState().config;
export const hasMedia = (): boolean => Boolean(player()?.media);

export const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

/** Rounds to `digits` decimals, avoiding float noise like 1.1000000000000001. */
export const round = (value: number, digits = 2) => {
  const f = 10 ** digits;
  return Math.round(value * f) / f;
};

export const formatDelay = (ms: number) => `${ms > 0 ? "+" : ""}${ms} ms`;
