/**
 * Audio, video, sleep-timer, and sidebar actions backed in Phase 5 (KEYBINDS_AND_CONFIG §2).
 * The sidebar tabs dispatch these same actions, so a key, a menu item, and a panel control
 * always do the same thing.
 */
import {
  AudioLines,
  Captions,
  Crop,
  FlipHorizontal2,
  FlipVertical2,
  Info,
  Moon,
  PanelRight,
  RectangleHorizontal,
  RotateCw,
  Scan,
  SlidersHorizontal,
  Sparkles,
  Speaker,
  Sun,
  Video,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import type { Renderer } from "@/ipc/bindings/Renderer";
import type { SleepKind } from "@/ipc/bindings/SleepKind";
import type { VideoPatch } from "@/ipc/bindings/VideoPatch";
import {
  audioSetDevice,
  audioSetEq,
  audioSetNormalize,
  playerSetSleepTimer,
  videoSetAdjust,
} from "@/ipc/commands";
import { useAppStore } from "@/stores/app";
import { eqFromPreset, useEqStore } from "@/stores/eq";
import { type SidebarTab, useUiStore } from "@/stores/ui";
import { clamp, config, hasMedia, player, round } from "../helpers";
import type { Action, Feedback, RunContext } from "../types";

// ── Video ────────────────────────────────────────────────────────────────────

export const ASPECTS: { value: string; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: "16:9", label: "16:9" },
  { value: "4:3", label: "4:3" },
  { value: "21:9", label: "21:9" },
  { value: "2.39", label: "2.39" },
  { value: "stretch", label: "Stretch" },
  { value: "fill", label: "Fill" },
];

/** Tone-mapping curves the renderer offers (ARCHITECTURE §6.6: some are gpu-next only). */
export function toneCurves(renderer: Renderer | undefined): { value: string; label: string }[] {
  const common = [
    { value: "auto", label: "Auto" },
    { value: "bt.2390", label: "BT.2390" },
    { value: "hable", label: "Hable (filmic)" },
    { value: "mobius", label: "Möbius" },
    { value: "reinhard", label: "Reinhard" },
    { value: "clip", label: "Clip" },
  ];
  if (renderer === "gpu") return common;
  return [
    ...common,
    { value: "spline", label: "Spline" },
    { value: "bt.2446a", label: "BT.2446a" },
    { value: "st2094-40", label: "ST 2094-40" },
    { value: "st2094-10", label: "ST 2094-10" },
  ];
}

const video = () => player()?.video;

export const zoomLabel = (zoom: number) => `${(2 ** zoom).toFixed(2)}×`;

async function adjust(patch: VideoPatch, feedback: Feedback): Promise<Feedback> {
  await videoSetAdjust(patch);
  return feedback;
}

const aspectLabel = (value: string) => ASPECTS.find((a) => a.value === value)?.label ?? value;

const videoActions: Action[] = [
  {
    id: "video.cycle_aspect",
    label: "Next aspect ratio",
    group: "video",
    icon: RectangleHorizontal,
    isEnabled: hasMedia,
    run: () => {
      const order = ASPECTS.filter((a) => a.value !== "fill").map((a) => a.value);
      const index = order.indexOf(video()?.aspect ?? "auto");
      const next = order[(index + 1) % order.length] ?? "auto";
      return adjust(
        { aspect: next },
        { icon: RectangleHorizontal, label: "Aspect", value: aspectLabel(next) },
      );
    },
  },
  {
    id: "video.set_aspect",
    label: "Set aspect ratio",
    group: "video",
    icon: RectangleHorizontal,
    isEnabled: hasMedia,
    run: (ctx: RunContext) => {
      const next = String(ctx.args?.value ?? "auto");
      return adjust(
        { aspect: next },
        { icon: RectangleHorizontal, label: "Aspect", value: aspectLabel(next) },
      );
    },
  },
  {
    id: "video.toggle_fill",
    label: "Crop to fill window",
    group: "video",
    icon: Crop,
    isEnabled: hasMedia,
    isOn: () => video()?.aspect === "fill",
    run: () => {
      const fill = video()?.aspect !== "fill";
      return adjust(
        { aspect: fill ? "fill" : (config()?.video.default_aspect ?? "auto") },
        { icon: Crop, label: fill ? "Fill window" : "Fit to window" },
      );
    },
  },
  {
    id: "video.rotate_cw",
    label: "Rotate 90°",
    group: "video",
    icon: RotateCw,
    isEnabled: hasMedia,
    run: () => {
      const next = ((video()?.rotate ?? 0) + 90) % 360;
      return adjust({ rotate: next }, { icon: RotateCw, label: "Rotation", value: `${next}°` });
    },
  },
  {
    id: "video.flip_horizontal",
    label: "Mirror",
    group: "video",
    icon: FlipHorizontal2,
    isEnabled: hasMedia,
    isOn: () => Boolean(video()?.flipH),
    run: () => {
      const on = !video()?.flipH;
      return adjust(
        { flipH: on },
        { icon: FlipHorizontal2, label: on ? "Mirrored" : "Mirror off" },
      );
    },
  },
  {
    id: "video.flip_vertical",
    label: "Flip",
    group: "video",
    icon: FlipVertical2,
    isEnabled: hasMedia,
    isOn: () => Boolean(video()?.flipV),
    run: () => {
      const on = !video()?.flipV;
      return adjust({ flipV: on }, { icon: FlipVertical2, label: on ? "Flipped" : "Flip off" });
    },
  },
  ...(
    [
      ["video.zoom_in", "Zoom in", ZoomIn, 1],
      ["video.zoom_out", "Zoom out", ZoomOut, -1],
    ] as const
  ).map(
    ([id, label, icon, dir]): Action => ({
      id,
      label,
      group: "video",
      icon,
      repeatable: true,
      isEnabled: hasMedia,
      run: () => {
        const step = config()?.controls.zoom_step ?? 0.1;
        const next = round(clamp((video()?.zoom ?? 0) + dir * step, -1, 2), 3);
        return adjust({ zoom: next }, { icon, label: "Zoom", value: zoomLabel(next) });
      },
    }),
  ),
  {
    id: "video.reset_transform",
    label: "Reset zoom, pan, rotation, flip",
    group: "video",
    icon: Scan,
    isEnabled: hasMedia,
    run: () =>
      adjust(
        { zoom: 0, panX: 0, panY: 0, rotate: 0, flipH: false, flipV: false },
        { icon: Scan, label: "Picture reset" },
      ),
  },
  {
    id: "video.toggle_deband",
    label: "Debanding on/off",
    group: "video",
    icon: Sparkles,
    isOn: () => Boolean(video()?.deband),
    run: () => {
      const on = !video()?.deband;
      return adjust(
        { deband: on },
        { icon: Sparkles, label: on ? "Debanding on" : "Debanding off" },
      );
    },
  },
  {
    id: "video.cycle_tone_mapping",
    label: "Next tone-mapping curve",
    group: "video",
    icon: Sun,
    run: () => {
      const curves = toneCurves(useAppStore.getState().info?.capabilities.renderer);
      const index = curves.findIndex((c) => c.value === video()?.toneMapping);
      const next = curves[(index + 1) % curves.length] ?? curves[0];
      if (!next) return undefined;
      return adjust(
        { toneMapping: next.value },
        { icon: Sun, label: "Tone mapping", value: next.label },
      );
    },
  },
];

// ── Audio ────────────────────────────────────────────────────────────────────

const audioActions: Action[] = [
  {
    id: "audio.cycle_device",
    label: "Next audio output device",
    group: "audio",
    icon: Speaker,
    isEnabled: () => (player()?.audioDevices.length ?? 0) > 1,
    run: async () => {
      const devices = player()?.audioDevices ?? [];
      const index = devices.findIndex((d) => d.name === player()?.audioDevice);
      const next = devices[(index + 1) % devices.length];
      if (!next) return undefined;
      await audioSetDevice(next.name);
      return { icon: Speaker, label: "Output", value: deviceLabel(next.name, next.description) };
    },
  },
  {
    id: "audio.toggle_equalizer",
    label: "Equalizer on/off",
    group: "audio",
    icon: SlidersHorizontal,
    isOn: () => Boolean(player()?.eq.enabled),
    run: async () => {
      const eq = player()?.eq;
      if (!eq) return undefined;
      await audioSetEq({ ...eq, enabled: !eq.enabled });
      return { icon: SlidersHorizontal, label: eq.enabled ? "Equalizer off" : "Equalizer on" };
    },
  },
  {
    id: "audio.set_equalizer_preset",
    label: "Set equalizer preset",
    group: "audio",
    icon: SlidersHorizontal,
    run: async (ctx: RunContext) => {
      const id = String(ctx.args?.id ?? "");
      const store = useEqStore.getState();
      const presets = store.loaded ? store.presets : await store.load();
      const preset = presets.find((p) => p.id === id);
      if (!preset) return { icon: SlidersHorizontal, label: "No preset", value: id };
      await audioSetEq(eqFromPreset(preset, true));
      return { icon: SlidersHorizontal, label: "Equalizer", value: preset.name };
    },
  },
  {
    id: "audio.toggle_normalize",
    label: "Volume normalisation on/off",
    group: "audio",
    icon: AudioLines,
    isOn: () => Boolean(player()?.normalize),
    run: async () => {
      const on = !player()?.normalize;
      await audioSetNormalize(on);
      return { icon: AudioLines, label: on ? "Normalisation on" : "Normalisation off" };
    },
  },
];

export function deviceLabel(name: string, description: string): string {
  if (name === "auto") return "System default";
  return description || name;
}

// ── Sleep timer ──────────────────────────────────────────────────────────────

export const SLEEP_STEPS: { kind: SleepKind; minutes?: number; label: string }[] = [
  { kind: "off", label: "Off" },
  { kind: "minutes", minutes: 15, label: "15 min" },
  { kind: "minutes", minutes: 30, label: "30 min" },
  { kind: "minutes", minutes: 45, label: "45 min" },
  { kind: "minutes", minutes: 60, label: "60 min" },
  { kind: "end_of_file", label: "End of file" },
  { kind: "end_of_queue", label: "End of queue" },
];

/** The step matching the current timer (by kind and length). */
export function currentSleepStep(): number {
  const sleep = player()?.sleep;
  if (!sleep) return 0;
  return Math.max(
    0,
    SLEEP_STEPS.findIndex(
      (s) => s.kind === sleep.kind && (s.kind !== "minutes" || s.minutes === sleep.minutes),
    ),
  );
}

async function setSleep(kind: SleepKind, minutes?: number): Promise<Feedback> {
  await playerSetSleepTimer({ kind, minutes });
  const label = SLEEP_STEPS.find((s) => s.kind === kind && s.minutes === minutes)?.label;
  return { icon: Moon, label: "Sleep timer", value: label ?? `${minutes} min` };
}

const sleepActions: Action[] = [
  {
    id: "player.sleep_timer_cycle",
    label: "Sleep timer",
    group: "playback",
    icon: Moon,
    isOn: () => Boolean(player()?.sleep),
    run: () => {
      // The keyboard cycle stops at "end of file" (KEYBINDS_AND_CONFIG §2).
      const cycle = SLEEP_STEPS.slice(0, 6);
      const next = cycle[(currentSleepStep() + 1) % cycle.length] ?? cycle[0];
      return next ? setSleep(next.kind, next.minutes) : undefined;
    },
  },
  {
    id: "player.set_sleep_timer",
    label: "Set sleep timer",
    group: "playback",
    icon: Moon,
    run: (ctx: RunContext) => {
      // `{ minutes }` sets a timed sleep; `{ kind }` also allows the end-of-file/queue modes.
      const kind = String(ctx.args?.kind ?? "minutes") as SleepKind;
      const minutes = ctx.args?.minutes === undefined ? undefined : Number(ctx.args.minutes);
      return setSleep(kind, minutes);
    },
  },
];

// ── Sidebar ──────────────────────────────────────────────────────────────────

const tabAction = (id: string, label: string, tab: SidebarTab, icon: Action["icon"]): Action => ({
  id,
  label,
  group: "window",
  icon,
  isOn: () => useUiStore.getState().dock === "sidebar" && useUiStore.getState().sidebarTab === tab,
  run: () => {
    useUiStore.getState().showSidebarTab(tab, true);
    return undefined;
  },
});

const sidebarActions: Action[] = [
  {
    id: "ui.toggle_sidebar",
    label: "Show sidebar",
    group: "window",
    icon: PanelRight,
    isOn: () => useUiStore.getState().dock === "sidebar",
    run: () => {
      useUiStore.getState().toggleDock("sidebar");
      return undefined;
    },
  },
  tabAction("ui.sidebar_subtitles", "Subtitles panel", "subtitles", Captions),
  tabAction("ui.sidebar_audio", "Audio panel", "audio", AudioLines),
  tabAction("ui.sidebar_video", "Video panel", "video", Video),
  tabAction("ui.sidebar_playback", "Playback panel", "playback", SlidersHorizontal),
  {
    id: "ui.toggle_inspector",
    label: "Stream info",
    group: "window",
    icon: Info,
    isOn: () =>
      useUiStore.getState().dock === "sidebar" && useUiStore.getState().sidebarTab === "playback",
    run: () => {
      const ui = useUiStore.getState();
      if (ui.dock === "sidebar" && ui.sidebarTab === "playback") {
        ui.setDock(null);
      } else {
        ui.showSidebarTab("playback");
        ui.requestInspector();
      }
      return undefined;
    },
  },
];

export const mediaActions: Action[] = [
  ...videoActions,
  ...audioActions,
  ...sleepActions,
  ...sidebarActions,
];
