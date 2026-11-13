import type { LucideIcon } from "lucide-react";

/** What triggered an action; decides whether an OSD flash is shown (KEYBINDS_AND_CONFIG §3.2). */
export type Trigger = "key" | "wheel" | "mouse" | "button" | "menu" | "palette";

export interface RunContext {
  trigger: Trigger;
  /** The binding that fired it, for the shortcut badge. */
  binding?: string;
  /** Arguments of a `[[custom]]` binding (§3.4). */
  args?: Record<string, unknown>;
  /** Whether this is a key auto-repeat. */
  repeat?: boolean;
}

/** What the OSD shows after an action ran from the keyboard or wheel. */
export interface Feedback {
  icon?: LucideIcon;
  label: string;
  value?: string;
  /** 0–1 for a mini bar (values above 1 show the volume boost zone). */
  bar?: number;
}

export type ActionGroup =
  | "playback"
  | "queue"
  | "audio"
  | "subtitles"
  | "video"
  | "window"
  | "app"
  | "context";

export type RunResult = Feedback | undefined;

export interface Action {
  id: string;
  label: string;
  group: ActionGroup;
  icon?: LucideIcon;
  run: (ctx: RunContext) => RunResult | Promise<RunResult>;
  /** Disabled actions do nothing at all — no OSD, no beep (§3.2 rule 5). */
  isEnabled?: () => boolean;
  /** For toggles: whether the state is currently on (menu check marks, button colour). */
  isOn?: () => boolean;
  /** Key auto-repeat allowed (steps: seek, volume, speed, delay, zoom). */
  repeatable?: boolean;
  /** Set on placeholders whose behaviour arrives in a later phase. */
  comingIn?: string;
}
