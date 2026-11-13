/**
 * Mirror of the core's queue (ARCHITECTURE §8.1), fed by `queue://changed`, plus the queue
 * panel's selection (view state: which rows are selected, the Shift-click anchor).
 */
import { create } from "zustand";
import type { QueueItem } from "@/ipc/bindings/QueueItem";
import type { QueueState } from "@/ipc/bindings/QueueState";

interface QueueStore {
  queue: QueueState | null;
  selected: string[];
  anchor: string | null;
  setQueue: (queue: QueueState) => void;
  /** Click: replace; Ctrl: toggle; Shift: range from the anchor. */
  select: (id: string, mode: "replace" | "toggle" | "range") => void;
  selectAll: () => void;
  clearSelection: () => void;
}

/** The ids from `anchor` to `id` inclusive, in list order. */
export function rangeIds(items: QueueItem[], anchor: string | null, id: string): string[] {
  const a = items.findIndex((i) => i.id === anchor);
  const b = items.findIndex((i) => i.id === id);
  if (a < 0 || b < 0) return [id];
  const [from, to] = a <= b ? [a, b] : [b, a];
  return items.slice(from, to + 1).map((i) => i.id);
}

export const useQueueStore = create<QueueStore>((set, get) => ({
  queue: null,
  selected: [],
  anchor: null,
  setQueue: (queue) => {
    const current = get().queue;
    if (current && queue.rev < current.rev) return;
    // Drop selections of items that no longer exist.
    const ids = new Set(queue.items.map((i) => i.id));
    set({ queue, selected: get().selected.filter((id) => ids.has(id)) });
  },
  select: (id, mode) => {
    const { selected, anchor, queue } = get();
    if (mode === "toggle") {
      set({
        selected: selected.includes(id) ? selected.filter((s) => s !== id) : [...selected, id],
        anchor: id,
      });
    } else if (mode === "range") {
      set({ selected: rangeIds(queue?.items ?? [], anchor ?? id, id) });
    } else {
      set({ selected: [id], anchor: id });
    }
  },
  selectAll: () => set({ selected: get().queue?.items.map((i) => i.id) ?? [] }),
  clearSelection: () => set({ selected: [], anchor: null }),
}));

/** Selected items in list order (actions operate on these). */
export function selectedItems(): QueueItem[] {
  const { queue, selected } = useQueueStore.getState();
  return (queue?.items ?? []).filter((i) => selected.includes(i.id));
}

/** "7 h 42 m" / "42 m" / "1 m". Unknown durations are left out. */
/** Total running time. Durations are learned as items load, so a partial sum gets a "+". */
export function formatTotal(items: QueueItem[]): string {
  const known = items.filter((i) => i.duration !== null);
  if (known.length === 0) return "";
  const seconds = known.reduce((sum, i) => sum + (i.duration ?? 0), 0);
  const minutes = Math.round(seconds / 60);
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  const total = h > 0 ? `${h} h ${m} m` : `${m} m`;
  return known.length < items.length ? `${total}+` : total;
}
