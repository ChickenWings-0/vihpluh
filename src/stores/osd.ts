/**
 * On-screen feedback (DESIGN_SYSTEM §8.9): one OSD flash at a time, replaced in place, and a
 * stack of up to three toasts. Toasts auto-dismiss after 5 s; errors stay until dismissed.
 */
import type { LucideIcon } from "lucide-react";
import { create } from "zustand";

export interface OsdFlash {
  id: number;
  icon?: LucideIcon;
  label: string;
  value?: string;
  /** 0–1 for a mini progress bar; above 1 shows the boost zone (volume). */
  bar?: number;
  /** The key cap that triggered it (shortcut badge). */
  badge?: string;
}

export interface Toast {
  id: number;
  kind: "info" | "warn" | "error" | "success";
  text: string;
  action?: { label: string; run: () => void };
}

const TOAST_LIMIT = 3;
const TOAST_MS = 5000;

interface OsdStore {
  flash: OsdFlash | null;
  toasts: Toast[];
  showFlash: (flash: Omit<OsdFlash, "id">, durationMs: number) => void;
  pushToast: (toast: Omit<Toast, "id">) => number;
  dismissToast: (id: number) => void;
}

let nextId = 1;
let flashTimer: ReturnType<typeof setTimeout> | undefined;

export const useOsdStore = create<OsdStore>((set, get) => ({
  flash: null,
  toasts: [],
  showFlash: (flash, durationMs) => {
    const id = nextId++;
    set({ flash: { ...flash, id } });
    if (flashTimer) clearTimeout(flashTimer);
    flashTimer = setTimeout(() => {
      if (get().flash?.id === id) set({ flash: null });
    }, durationMs);
  },
  pushToast: (toast) => {
    const id = nextId++;
    // Newest at the end; the oldest is dropped past the limit.
    set({ toasts: [...get().toasts, { ...toast, id }].slice(-TOAST_LIMIT) });
    if (toast.kind !== "error") {
      setTimeout(() => get().dismissToast(id), TOAST_MS);
    }
    return id;
  },
  dismissToast: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
}));

/** Reports a failed command as an error toast; the message comes from the core. */
export function reportError(error: unknown): void {
  const message =
    typeof error === "object" && error !== null && "message" in error
      ? String((error as { message: unknown }).message)
      : String(error);
  console.error(error);
  useOsdStore.getState().pushToast({ kind: "error", text: message });
}
