/**
 * The only module that imports `@tauri-apps/api` (ARCHITECTURE §4.5). Everything else talks to
 * the core through `commands.ts` and `events.ts`, which keeps the transport swappable.
 */
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
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
