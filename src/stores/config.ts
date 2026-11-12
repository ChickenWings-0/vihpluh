/** The effective configuration, kept current by `config://changed` (KEYBINDS_AND_CONFIG §6). */
import { create } from "zustand";
import type { Config } from "@/ipc/bindings/Config";

interface ConfigStore {
  config: Config | null;
  setConfig: (config: Config) => void;
  /** Applies one changed key (dotted path, snake_case segments as in `config.toml`). */
  applyChange: (path: string, value: unknown) => void;
}

/** Returns a copy of `root` with the value at `path` replaced (or removed when null). */
export function setAtPath<T extends object>(root: T, path: string, value: unknown): T {
  // Keys under mpv.options may themselves contain no dots, but the table names do.
  const freeForm = ["mpv.options", "env"].find((prefix) => path.startsWith(`${prefix}.`));
  const parts = freeForm
    ? [...freeForm.split("."), path.slice(freeForm.length + 1)]
    : path.split(".");
  const update = (node: unknown, index: number): unknown => {
    const key = parts[index];
    if (key === undefined) return value;
    const object = typeof node === "object" && node !== null ? node : {};
    const copy: Record<string, unknown> = { ...(object as Record<string, unknown>) };
    if (index === parts.length - 1 && value === null) {
      delete copy[key];
    } else {
      copy[key] = update(copy[key], index + 1);
    }
    return copy;
  };
  return update(root, 0) as T;
}

export const useConfigStore = create<ConfigStore>((set, get) => ({
  config: null,
  setConfig: (config) => set({ config }),
  applyChange: (path, value) => {
    const current = get().config;
    if (current) set({ config: setAtPath(current, path, value) });
  },
}));

/** Narrow selector helper: `useConfig((c) => c.interface.accent)`. */
export function useConfig<T>(select: (config: Config) => T): T | undefined {
  return useConfigStore((store) => (store.config ? select(store.config) : undefined));
}
