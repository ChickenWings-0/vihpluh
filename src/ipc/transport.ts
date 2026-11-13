/**
 * The only module that imports `@tauri-apps/api` (ARCHITECTURE §4.5). Everything else talks to
 * the core through `commands.ts` and `events.ts`, which keeps the transport swappable.
 */
import { convertFileSrc, invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { AppError } from "./bindings/AppError";

export type Unlisten = () => void;

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { code?: unknown }).code === "string" &&
    typeof (value as { message?: unknown }).message === "string"
  );
}

/** Calls a core command. Rejections are always `AppError`. */
export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    if (isAppError(error)) throw error;
    const failure: AppError = { code: "internal", message: String(error) };
    throw failure;
  }
}

/** Subscribes to a core event. The returned function unsubscribes and is safe to call early. */
export function listen<T>(event: string, handler: (payload: T) => void): Unlisten {
  let cancelled = false;
  let unlisten: Unlisten | undefined;
  tauriListen<T>(event, (e) => handler(e.payload)).then((fn) => {
    if (cancelled) fn();
    else unlisten = fn;
  });
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

// ── Window helpers (direct Tauri window API; ARCHITECTURE §7.3 last line) ───────────────────

export type ResizeEdge =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

/** Starts an OS window move; call from a primary-button pointerdown/move. */
export function startWindowDrag(): Promise<void> {
  return getCurrentWindow().startDragging();
}

/** Starts an OS window resize from one edge or corner. */
export function startWindowResize(edge: ResizeEdge): Promise<void> {
  return getCurrentWindow().startResizeDragging(edge);
}

export type DropEvent =
  | { type: "enter" | "over"; x: number; y: number }
  | { type: "drop"; x: number; y: number; paths: string[] }
  | { type: "leave" };

/** Files dragged from the OS onto the window. Positions are in CSS pixels. */
export function onFileDrop(handler: (event: DropEvent) => void): Unlisten {
  let cancelled = false;
  let unlisten: Unlisten | undefined;
  const scale = () => window.devicePixelRatio || 1;
  getCurrentWebview()
    .onDragDropEvent(({ payload }) => {
      if (payload.type === "leave") {
        handler({ type: "leave" });
        return;
      }
      const x = payload.position.x / scale();
      const y = payload.position.y / scale();
      if (payload.type === "drop") handler({ type: "drop", x, y, paths: payload.paths });
      else handler({ type: payload.type, x, y });
    })
    .then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

// ── Custom URI schemes (ARCHITECTURE §7.5) ─────────────────────────────────────────────────

let thumbBase: string | null = null;

/**
 * URL of a seek-preview thumbnail. The webview spells custom schemes differently per platform
 * (`http://vthumb.localhost/` on Windows, `vthumb://localhost/` elsewhere); `convertFileSrc`
 * knows which, so its prefix is reused.
 */
export function thumbUrl(mediaKey: string, step: number, bucket: number): string {
  if (thumbBase === null) {
    const probe = convertFileSrc("x", "vthumb");
    thumbBase = probe.slice(0, probe.length - 1);
  }
  return `${thumbBase}${encodeURIComponent(mediaKey)}/${step}/${bucket}.jpg`;
}
