/**
 * Connects the stores to the core: subscribes first, then fetches snapshots, so nothing that
 * happens in between is lost. A missed patch (revision gap) triggers a fresh snapshot.
 */
import { appInfo, configGet, keybindsGet, playerSnapshot, windowGetState } from "@/ipc/commands";
import {
  onConfigChanged,
  onKeybindsChanged,
  onOsd,
  onPlayerPatch,
  onPlayerTime,
  onWindowState,
} from "@/ipc/events";
import { useAppStore } from "./app";
import { useConfigStore } from "./config";
import { useKeymapStore } from "./keymap";
import { reportError, useOsdStore } from "./osd";
import { usePlayerStore } from "./player";
import { receiveTime, setClock } from "./time";
import { useUiStore } from "./ui";

export function startSync(): () => void {
  let refetching = false;
  const refetch = () => {
    if (refetching) return;
    refetching = true;
    playerSnapshot()
      .then((snapshot) => usePlayerStore.getState().setSnapshot(snapshot))
      .catch(reportError)
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
    onKeybindsChanged((keymap) => useKeymapStore.getState().setKeymap(keymap)),
    onWindowState((state) => useUiStore.getState().setWindow(state)),
    // Core notices need reading or carry an action: they are toasts (DESIGN_SYSTEM §8.9).
    onOsd((osd) => {
      useOsdStore.getState().pushToast({ kind: osd.kind, text: osd.text });
    }),
  ];

  // The interpolation clock follows status, speed, and direction.
  const unsubscribeClock = usePlayerStore.subscribe((store, previous) => {
    const state = store.state;
    if (!state) return;
    const before = previous.state;
    if (
      before &&
      before.status === state.status &&
      before.speed === state.speed &&
      before.direction === state.direction
    ) {
      return;
    }
    const forward = state.direction === "forward" ? 1 : -1;
    setClock(state.status === "playing", state.speed * forward);
  });

  refetch();
  configGet()
    .then((config) => useConfigStore.getState().setConfig(config))
    .catch(reportError);
  keybindsGet()
    .then((keymap) => useKeymapStore.getState().setKeymap(keymap))
    .catch(reportError);
  windowGetState()
    .then((state) => useUiStore.getState().setWindow(state))
    .catch(reportError);
  appInfo()
    .then((info) => useAppStore.getState().setInfo(info))
    .catch(reportError);

  return () => {
    for (const unlisten of unlisteners) unlisten();
    unsubscribeClock();
  };
}
