/**
 * Applies the look settings to the document (DESIGN_SYSTEM §2.2–2.3, §12): the theme
 * (`system` follows the OS), the accent and everything derived from it, the interface scale,
 * and reduced motion. Canvases read `resolved` and `accent` to know when to redraw.
 */
import { create } from "zustand";
import { DEFAULT_ACCENT, deriveAccent, isHex } from "@/lib/color";
import { useConfigStore } from "./config";

export type ResolvedTheme = "dark" | "light";

interface ThemeStore {
  /** The theme in effect, with `system` resolved. */
  resolved: ResolvedTheme;
  accent: string;
  reduceMotion: boolean;
}

export const useThemeStore = create<ThemeStore>(() => ({
  resolved: "dark",
  accent: DEFAULT_ACCENT,
  reduceMotion: false,
}));

/** Root font size at scale 1; every dimension is in rem, so this scales the interface. */
const BASE_FONT_PX = 16;

export function resolveTheme(setting: string | undefined, systemLight: boolean): ResolvedTheme {
  if (setting === "light") return "light";
  if (setting === "system") return systemLight ? "light" : "dark";
  return "dark";
}

export function resolveReduceMotion(setting: string | undefined, systemReduce: boolean): boolean {
  if (setting === "on") return true;
  if (setting === "off") return false;
  return systemReduce;
}

export function startTheme(): () => void {
  const root = document.documentElement;
  const light = window.matchMedia("(prefers-color-scheme: light)");
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");

  const apply = () => {
    const ui = useConfigStore.getState().config?.interface;
    const resolved = resolveTheme(ui?.theme, light.matches);
    const accent = ui && isHex(ui.accent) ? ui.accent.toUpperCase() : DEFAULT_ACCENT;
    const reduceMotion = resolveReduceMotion(ui?.reduce_motion, reduce.matches);

    root.dataset.theme = resolved;
    root.dataset.reduceMotion = String(reduceMotion);
    root.style.colorScheme = resolved;
    // The shipped orange keeps the hand-tuned tokens of the stylesheet (§2.1, §2.2); any
    // other accent has its tokens derived (§2.3).
    for (const [token, value] of Object.entries(deriveAccent(accent, resolved))) {
      if (accent === DEFAULT_ACCENT) root.style.removeProperty(token);
      else root.style.setProperty(token, value);
    }
    const scale = Math.min(1.5, Math.max(0.8, ui?.ui_scale ?? 1));
    root.style.fontSize = `${BASE_FONT_PX * scale}px`;

    const now = useThemeStore.getState();
    if (now.resolved !== resolved || now.accent !== accent || now.reduceMotion !== reduceMotion) {
      useThemeStore.setState({ resolved, accent, reduceMotion });
    }
  };

  apply();
  const unsubscribe = useConfigStore.subscribe((store, previous) => {
    if (store.config?.interface !== previous.config?.interface) apply();
  });
  light.addEventListener("change", apply);
  reduce.addEventListener("change", apply);
  return () => {
    unsubscribe();
    light.removeEventListener("change", apply);
    reduce.removeEventListener("change", apply);
  };
}
