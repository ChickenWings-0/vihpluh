/** Playback actions (KEYBINDS_AND_CONFIG §2 "Playback"). */
import {
  Camera,
  CircleStop,
  Clock,
  FastForward,
  Gauge,
  Pause,
  Play,
  Repeat,
  Rewind,
  StepBack,
  StepForward,
  Volume2,
  VolumeX,
} from "lucide-react";
import {
  playerFrameStep,
  playerScreenshot,
  playerSeek,
  playerSetAbLoop,
  playerSetDirection,
  playerSetMuted,
  playerSetSpeed,
  playerSetVolume,
  playerStop,
  playerTogglePause,
} from "@/ipc/commands";
import { formatTime } from "@/lib/time";
import { useOsdStore } from "@/stores/osd";
import { getDuration, getPosition } from "@/stores/time";
import { useUiStore } from "@/stores/ui";
import { clamp, config, hasMedia, player, round } from "../helpers";
import type { Action, Feedback, RunContext } from "../types";

const seekFeedback = (target: number): Feedback => {
  const dur = getDuration();
  const pos = dur > 0 ? clamp(target, 0, dur) : Math.max(0, target);
  return {
    icon: pos >= getPosition() ? FastForward : Rewind,
    label: "Seek",
    value: `${formatTime(pos)} / ${formatTime(dur)}`,
    bar: dur > 0 ? pos / dur : undefined,
  };
};

async function seekBy(seconds: number, exact: boolean): Promise<Feedback> {
  await playerSeek({ kind: "relative", value: seconds, exact });
  return seekFeedback(getPosition() + seconds);
}

const preciseSeek = () => config()?.controls.precise_seek ?? true;

function seekAction(id: string, label: string, step: () => number, exact?: boolean): Action {
  return {
    id,
    label,
    group: "playback",
    icon: step() < 0 ? Rewind : FastForward,
    repeatable: true,
    isEnabled: hasMedia,
    run: () => seekBy(step(), exact ?? preciseSeek()),
  };
}

const steps = () => config()?.controls;

async function setSpeed(value: number): Promise<Feedback> {
  const speed = round(clamp(value, 0.25, 4), 2);
  await playerSetSpeed(speed);
  return { icon: Gauge, label: "Speed", value: `${speed.toFixed(2)}×` };
}

async function setVolume(value: number): Promise<Feedback> {
  const max = config()?.audio.max_volume ?? 130;
  const volume = Math.round(clamp(value, 0, max));
  await playerSetVolume(volume);
  if (player()?.muted) await playerSetMuted(false);
  return { icon: Volume2, label: "Volume", value: `${volume} %`, bar: volume / 100 };
}

const volumeAction = (id: string, label: string, sign: 1 | -1, fine: boolean): Action => ({
  id,
  label,
  group: "playback",
  icon: Volume2,
  repeatable: true,
  run: () => {
    const step = fine ? 1 : (steps()?.volume_step ?? 5);
    return setVolume((player()?.volume ?? 100) + sign * step);
  },
});

const percentActions: Action[] = Array.from({ length: 10 }, (_, n) => ({
  id: `player.seek_percent_${n}`,
  label: `Jump to ${n * 10} %`,
  group: "playback",
  isEnabled: hasMedia,
  run: async () => {
    await playerSeek({ kind: "percent", value: n * 10, exact: preciseSeek() });
    return seekFeedback((getDuration() * n) / 10);
  },
}));

export const playbackActions: Action[] = [
  {
    id: "player.toggle_pause",
    label: "Play / Pause",
    group: "playback",
    icon: Play,
    isEnabled: hasMedia,
    isOn: () => player()?.status === "playing",
    run: async () => {
      const wasPlaying = player()?.status === "playing";
      await playerTogglePause();
      return { icon: wasPlaying ? Pause : Play, label: wasPlaying ? "Paused" : "Playing" };
    },
  },
  {
    id: "player.stop",
    label: "Stop and close media",
    group: "playback",
    icon: CircleStop,
    isEnabled: hasMedia,
    run: async () => {
      await playerStop();
      return { icon: CircleStop, label: "Stopped" };
    },
  },
  seekAction("player.seek_back_small", "Seek back", () => -(steps()?.seek_small_s ?? 5)),
  seekAction("player.seek_forward_small", "Seek forward", () => steps()?.seek_small_s ?? 5),
  seekAction("player.seek_back_medium", "Seek back more", () => -(steps()?.seek_medium_s ?? 10)),
  seekAction("player.seek_forward_medium", "Seek forward more", () => steps()?.seek_medium_s ?? 10),
  seekAction("player.seek_back_large", "Seek back a lot", () => -(steps()?.seek_large_s ?? 60)),
  seekAction("player.seek_forward_large", "Seek forward a lot", () => steps()?.seek_large_s ?? 60),
  seekAction("player.seek_back_exact", "Seek back 1 s, frame-exact", () => -1, true),
  seekAction("player.seek_forward_exact", "Seek forward 1 s, frame-exact", () => 1, true),
  ...percentActions,
  {
    id: "player.seek_start",
    label: "Jump to start",
    group: "playback",
    icon: Rewind,
    isEnabled: hasMedia,
    run: async () => {
      await playerSeek({ kind: "absolute", value: 0, exact: true });
      return seekFeedback(0);
    },
  },
  {
    id: "player.frame_step_back",
    label: "Previous frame",
    group: "playback",
    icon: StepBack,
    repeatable: true,
    isEnabled: hasMedia,
    run: async () => {
      await playerFrameStep("backward");
      return { icon: StepBack, label: "Previous frame" };
    },
  },
  {
    id: "player.frame_step_forward",
    label: "Next frame",
    group: "playback",
    icon: StepForward,
    repeatable: true,
    isEnabled: hasMedia,
    run: async () => {
      await playerFrameStep("forward");
      return { icon: StepForward, label: "Next frame" };
    },
  },
  {
    id: "player.speed_down",
    label: "Slower",
    group: "playback",
    icon: Gauge,
    repeatable: true,
    run: () => setSpeed((player()?.speed ?? 1) - (steps()?.speed_step ?? 0.1)),
  },
  {
    id: "player.speed_up",
    label: "Faster",
    group: "playback",
    icon: Gauge,
    repeatable: true,
    run: () => setSpeed((player()?.speed ?? 1) + (steps()?.speed_step ?? 0.1)),
  },
  {
    id: "player.speed_halve",
    label: "Half speed",
    group: "playback",
    icon: Gauge,
    run: () => setSpeed((player()?.speed ?? 1) / 2),
  },
  {
    id: "player.speed_double",
    label: "Double speed",
    group: "playback",
    icon: Gauge,
    run: () => setSpeed((player()?.speed ?? 1) * 2),
  },
  {
    id: "player.speed_reset",
    label: "Normal speed",
    group: "playback",
    icon: Gauge,
    run: () => setSpeed(1),
  },
  {
    id: "player.toggle_reverse",
    label: "Reverse playback",
    group: "playback",
    icon: Rewind,
    isEnabled: hasMedia,
    isOn: () => player()?.direction === "backward",
    run: async () => {
      const backward = player()?.direction !== "backward";
      await playerSetDirection(backward ? "backward" : "forward");
      return { icon: Rewind, label: backward ? "Reverse playback" : "Forward playback" };
    },
  },
  volumeAction("player.volume_up", "Volume up", 1, false),
  volumeAction("player.volume_down", "Volume down", -1, false),
  volumeAction("player.volume_up_fine", "Volume up a little", 1, true),
  volumeAction("player.volume_down_fine", "Volume down a little", -1, true),
  {
    id: "player.toggle_mute",
    label: "Mute",
    group: "playback",
    icon: VolumeX,
    isOn: () => Boolean(player()?.muted),
    run: async () => {
      const muted = !player()?.muted;
      await playerSetMuted(muted);
      return muted
        ? { icon: VolumeX, label: "Muted" }
        : { icon: Volume2, label: "Volume", value: `${Math.round(player()?.volume ?? 0)} %` };
    },
  },
  {
    id: "player.ab_loop_cycle",
    label: "A–B loop: set A, set B, clear",
    group: "playback",
    icon: Repeat,
    isEnabled: hasMedia,
    isOn: () => player()?.abLoop.a !== null && player()?.abLoop.a !== undefined,
    run: async () => {
      const loop = player()?.abLoop ?? { a: null, b: null };
      const now = round(getPosition(), 3);
      if (loop.a === null) {
        await playerSetAbLoop({ a: now, b: null });
        return { icon: Repeat, label: "Loop start (A)", value: formatTime(now) };
      }
      if (loop.b === null) {
        const [a, b] = now >= loop.a ? [loop.a, now] : [now, loop.a];
        await playerSetAbLoop({ a, b });
        return { icon: Repeat, label: "Loop A–B", value: `${formatTime(a)} – ${formatTime(b)}` };
      }
      await playerSetAbLoop({ a: null, b: null });
      return { icon: Repeat, label: "Loop off" };
    },
  },
  {
    id: "player.screenshot",
    label: "Screenshot with subtitles",
    group: "playback",
    icon: Camera,
    isEnabled: hasMedia,
    run: () => screenshot(true),
  },
  {
    id: "player.screenshot_clean",
    label: "Screenshot without subtitles",
    group: "playback",
    icon: Camera,
    isEnabled: hasMedia,
    run: () => screenshot(false),
  },
  {
    id: "player.show_progress",
    label: "Show time and progress",
    group: "playback",
    icon: Clock,
    isEnabled: hasMedia,
    run: () => {
      useUiStore.getState().requestReveal();
      return seekFeedback(getPosition());
    },
  },
  // ── Parameterised: [[custom]] bindings only (§3.4) ──────────────────────────
  {
    id: "player.seek_relative",
    label: "Seek by a set amount",
    group: "playback",
    icon: FastForward,
    repeatable: true,
    isEnabled: hasMedia,
    run: (ctx: RunContext) => {
      const seconds = Number(ctx.args?.seconds ?? 0);
      const exact = ctx.args?.exact === undefined ? preciseSeek() : Boolean(ctx.args.exact);
      return seekBy(seconds, exact);
    },
  },
  {
    id: "player.seek_absolute",
    label: "Jump to a set time",
    group: "playback",
    icon: FastForward,
    isEnabled: hasMedia,
    run: async (ctx: RunContext) => {
      if (ctx.args?.percent !== undefined) {
        const percent = Number(ctx.args.percent);
        await playerSeek({ kind: "percent", value: percent, exact: preciseSeek() });
        return seekFeedback((getDuration() * percent) / 100);
      }
      const seconds = Number(ctx.args?.seconds ?? 0);
      await playerSeek({ kind: "absolute", value: seconds, exact: preciseSeek() });
      return seekFeedback(seconds);
    },
  },
  {
    id: "player.set_speed",
    label: "Set speed",
    group: "playback",
    icon: Gauge,
    run: (ctx: RunContext) => setSpeed(Number(ctx.args?.value ?? 1)),
  },
  {
    id: "player.set_volume",
    label: "Set volume",
    group: "playback",
    icon: Volume2,
    run: (ctx: RunContext) => setVolume(Number(ctx.args?.value ?? 100)),
  },
];

async function screenshot(subtitles: boolean): Promise<Feedback> {
  const path = await playerScreenshot(subtitles);
  useOsdStore.getState().pushToast({ kind: "success", text: `Screenshot saved: ${path}` });
  return { icon: Camera, label: "Screenshot saved" };
}
