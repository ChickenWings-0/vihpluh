/**
 * Connects the stores to the core: subscribes first, then fetches snapshots, so nothing that
 * happens in between is lost. A missed patch (revision gap) triggers a fresh snapshot.
 */
import { dispatch } from "@/actions/registry";
import {
  appInfo,
  configDefaults,
  configGet,
  keybindsGet,
  playerSnapshot,
  queueSnapshot,
  sessionUiGet,
  sessionUiSet,
  windowGetState,
} from "@/ipc/commands";
import {
  onConfigChanged,
  onKeybindsChanged,
  onOsd,
  onPlayerPatch,
  onPlayerTime,
  onQueueChanged,
  onWindowState,
} from "@/ipc/events";
import { loadKeyboardLayout } from "@/keybinds/layout";
import { useAppStore } from "./app";
import { startBookmarkSync } from "./bookmarks";
import { useConfigStore } from "./config";
import { useKeymapStore } from "./keymap";
import { reportError, useOsdStore } from "./osd";
import { usePlayerStore } from "./player";
import { startPresetSync } from "./presets";
import { useQueueStore } from "./queue";
import { startTheme } from "./theme";
import { receiveTime, resetTime, setClock } from "./time";
import { setUiPersister, useUiStore } from "./ui";

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
    onQueueChanged((queue) => useQueueStore.getState().setQueue(queue)),
    // Core notices need reading or carry an action: they are toasts (DESIGN_SYSTEM §8.9).
    onOsd((osd) => {
      const action = osd.action;
      useOsdStore.getState().pushToast({
        kind: osd.kind,
        text: osd.text,
        action: action
          ? {
              label: action.label,
              run: () =>
                void dispatch(action.actionId, {
                  trigger: "button",
                  args: action.args as Record<string, unknown> | undefined,
                }),
            }
          : undefined,
        group: osd.group,
      });
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
    // Nothing loaded: the bar must not keep showing the last file's time and buffer.
    if (state.status === "idle") {
      resetTime();
      return;
    }
    const forward = state.direction === "forward" ? 1 : -1;
    setClock(state.status === "playing", state.speed * forward);
  });

  const stopBookmarks = startBookmarkSync();
  const stopPresets = startPresetSync();
  const stopTheme = startTheme();
  setUiPersister((patch) => void sessionUiSet(patch).catch(reportError));
  loadKeyboardLayout();
  refetch();
  queueSnapshot()
    .then((queue) => useQueueStore.getState().setQueue(queue))
    .catch(reportError);
  sessionUiGet()
    .then((ui) => useUiStore.getState().hydrate(ui))
    .catch(reportError);
  configGet()
    .then((config) => useConfigStore.getState().setConfig(config))
    .catch(reportError);
  configDefaults()
    .then((defaults) => useConfigStore.getState().setDefaults(defaults))
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
    stopBookmarks();
    stopPresets();
    stopTheme();
  };
}
