/** Queue actions (KEYBINDS_AND_CONFIG §2 "Queue…" and the `queue` context). */
import {
  CircleStop,
  FolderOpen,
  ListX,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Trash2,
} from "lucide-react";
import type { QueueItem } from "@/ipc/bindings/QueueItem";
import type { Repeat as RepeatMode } from "@/ipc/bindings/Repeat";
import {
  appReveal,
  queueAdd,
  queueClear,
  queueMove,
  queueNext,
  queuePlay,
  queuePrev,
  queueRemove,
  queueSetRepeat,
  queueSetShuffle,
  queueSetStopAfterCurrent,
} from "@/ipc/commands";
import { reportError, useOsdStore } from "@/stores/osd";
import { selectedItems, useQueueStore } from "@/stores/queue";
import { useUiStore } from "@/stores/ui";
import { player } from "../helpers";
import type { Action } from "../types";

const queue = () => useQueueStore.getState().queue;
const items = () => queue()?.items ?? [];
const hasSelection = () => useQueueStore.getState().selected.length > 0;

const REPEAT_ORDER: RepeatMode[] = ["off", "all", "one"];
const REPEAT_LABEL: Record<RepeatMode, string> = {
  off: "Repeat off",
  all: "Repeat all",
  one: "Repeat one",
};

/**
 * Removes items with an Undo toast (no confirmation dialogs for reversible actions,
 * DESIGN_SYSTEM §1). Undo puts each item back where it was.
 */
export async function removeWithUndo(removed: QueueItem[]): Promise<void> {
  if (removed.length === 0) return;
  const all = items();
  const positions = removed.map((item) => ({
    item,
    index: all.findIndex((i) => i.id === item.id),
  }));
  await queueRemove(removed.map((i) => i.id));
  const text =
    removed.length === 1 ? `Removed ${removed[0]?.title}` : `Removed ${removed.length} items`;
  useOsdStore.getState().pushToast({
    kind: "info",
    text,
    action: {
      label: "Undo",
      run: () => {
        // Re-insert from the lowest index up, so the original positions line up again.
        const sorted = [...positions].sort((a, b) => a.index - b.index);
        void (async () => {
          for (const { item, index } of sorted) await queueAdd([item.source], index);
        })().catch(reportError);
      },
    },
  });
}

export const queueActions: Action[] = [
  {
    id: "queue.next",
    label: "Next item",
    group: "queue",
    icon: SkipForward,
    isEnabled: () => Boolean(queue()?.canNext),
    run: async () => {
      await queueNext();
      return { icon: SkipForward, label: "Next" };
    },
  },
  {
    id: "queue.previous",
    label: "Previous item",
    group: "queue",
    icon: SkipBack,
    isEnabled: () => Boolean(queue()?.canPrevious),
    run: async () => {
      await queuePrev();
      return { icon: SkipBack, label: "Previous" };
    },
  },
  {
    id: "queue.toggle_panel",
    label: "Show queue",
    group: "queue",
    icon: ListX,
    isOn: () => useUiStore.getState().dock === "queue",
    run: () => {
      useUiStore.getState().toggleDock("queue");
      return undefined;
    },
  },
  {
    id: "queue.cycle_repeat",
    label: "Repeat: off, all, one",
    group: "queue",
    icon: Repeat,
    isOn: () => (queue()?.repeat ?? "off") !== "off",
    run: async () => {
      const current = queue()?.repeat ?? "off";
      const next = REPEAT_ORDER[(REPEAT_ORDER.indexOf(current) + 1) % REPEAT_ORDER.length] ?? "off";
      await queueSetRepeat(next);
      return { icon: next === "one" ? Repeat1 : Repeat, label: REPEAT_LABEL[next] };
    },
  },
  {
    id: "queue.toggle_shuffle",
    label: "Shuffle",
    group: "queue",
    icon: Shuffle,
    isOn: () => Boolean(queue()?.shuffle),
    run: async () => {
      const on = !queue()?.shuffle;
      await queueSetShuffle(on);
      return { icon: Shuffle, label: on ? "Shuffle on" : "Shuffle off" };
    },
  },
  {
    id: "queue.stop_after_current",
    label: "Stop after this item",
    group: "queue",
    icon: CircleStop,
    isEnabled: () => Boolean(queue()?.currentId),
    isOn: () => Boolean(queue()?.stopAfterCurrent),
    run: async () => {
      const on = !queue()?.stopAfterCurrent;
      await queueSetStopAfterCurrent(on);
      return { icon: CircleStop, label: on ? "Stop after this item" : "Keep playing" };
    },
  },
  {
    id: "queue.clear",
    label: "Clear queue",
    group: "queue",
    icon: Trash2,
    isEnabled: () => items().length > 1 || (items().length === 1 && !queue()?.currentId),
    run: async () => {
      const keep = queue()?.currentId ?? null;
      const removed = items().filter((i) => i.id !== keep);
      await queueClear(true);
      useOsdStore.getState().pushToast({
        kind: "info",
        text: `Cleared ${removed.length} items`,
        action: {
          label: "Undo",
          run: () => void queueAdd(removed.map((i) => i.source)).catch(reportError),
        },
      });
      return undefined;
    },
  },
  {
    id: "app.reveal_current",
    label: "Show current file in file manager",
    group: "app",
    icon: FolderOpen,
    isEnabled: () => {
      const media = player()?.media;
      return Boolean(media && !media.isUrl);
    },
    run: async () => {
      const source = player()?.media?.source;
      if (source) await appReveal(source);
      return undefined;
    },
  },
  // ── Queue panel context (active while focus is in the panel) ─────────────
  {
    id: "queue.play_selected",
    label: "Play",
    group: "context",
    icon: Play,
    isEnabled: hasSelection,
    run: async () => {
      const first = selectedItems()[0];
      if (first) await queuePlay(first.id);
      return undefined;
    },
  },
  {
    id: "queue.remove_selected",
    label: "Remove",
    group: "context",
    icon: Trash2,
    isEnabled: hasSelection,
    run: async () => {
      await removeWithUndo(selectedItems());
      return undefined;
    },
  },
  {
    id: "queue.move_selected_up",
    label: "Move up",
    group: "context",
    repeatable: true,
    isEnabled: hasSelection,
    run: async () => {
      const chosen = selectedItems();
      const first = items().findIndex((i) => i.id === chosen[0]?.id);
      if (first > 0)
        await queueMove(
          chosen.map((i) => i.id),
          first - 1,
        );
      return undefined;
    },
  },
  {
    id: "queue.move_selected_down",
    label: "Move down",
    group: "context",
    repeatable: true,
    isEnabled: hasSelection,
    run: async () => {
      const chosen = selectedItems();
      const all = items();
      const ids = new Set(chosen.map((i) => i.id));
      const rest = all.filter((i) => !ids.has(i.id));
      const firstIndex = all.findIndex((i) => ids.has(i.id));
      // Position of the first selected item among the others, plus one.
      const before = all.slice(0, firstIndex).filter((i) => !ids.has(i.id)).length;
      if (before < rest.length) await queueMove([...ids], before + 1);
      return undefined;
    },
  },
  {
    id: "queue.select_all",
    label: "Select all",
    group: "context",
    run: () => {
      useQueueStore.getState().selectAll();
      return undefined;
    },
  },
];
