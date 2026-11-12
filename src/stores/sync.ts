/**
 * Connects the stores to the core: subscribes first, then fetches snapshots, so nothing that
 * happens in between is lost. A missed patch (revision gap) triggers a fresh snapshot.
 */
import { configGet, playerSnapshot } from "@/ipc/commands";
import { onConfigChanged, onPlayerPatch, onPlayerTime } from "@/ipc/events";
import { useConfigStore } from "./config";
import { usePlayerStore } from "./player";
import { receiveTime, setClock } from "./time";

export function startSync(): () => void {
  let refetching = false;
  const refetch = () => {
    if (refetching) return;
    refetching = true;
    playerSnapshot()
      .then((snapshot) => usePlayerStore.getState().setSnapshot(snapshot))
      .catch((error: unknown) => console.error("player_snapshot failed", error))
      .finally(() => {
        refetching = false;
      });
  };

  const unlisteners = [
    onPlayerPatch((patch) => {
      if (usePlayerStore.getState().applyPatch(patch) === "gap") refetch();
    }),
    onPlayerTime((event) => receiveTime(event)),
    onConfigChanged(({ path, value }) => useConfigStore.getState().applyChange(path, value)),
  ];

  // The interpolation clock follows status and speed.
  const unsubscribeClock = usePlayerStore.subscribe((store, previous) => {
    const state = store.state;
    if (!state) return;
    const before = previous.state;
    if (before && before.status === state.status && before.speed === state.speed) return;
    const forward = state.direction === "forward" ? 1 : -1;
    setClock(state.status === "playing", state.speed * forward);
  });

  refetch();
  configGet()
    .then((config) => useConfigStore.getState().setConfig(config))
    .catch((error: unknown) => console.error("config_get failed", error));

  return () => {
    for (const unlisten of unlisteners) unlisten();
    unsubscribeClock();
  };
}
