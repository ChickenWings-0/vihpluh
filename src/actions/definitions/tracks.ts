/** Audio and subtitle actions that the core can back today (KEYBINDS_AND_CONFIG §2). */
import { AudioLines, Captions, Clock, Eye, EyeOff, Terminal } from "lucide-react";
import type { Track } from "@/ipc/bindings/Track";
import {
  audioSetDelay,
  playerMpvCommand,
  subSetDelay,
  subSetVisible,
  tracksSelect,
} from "@/ipc/commands";
import { config, formatDelay, hasMedia, player } from "../helpers";
import type { Action, Feedback, RunContext } from "../types";

export function trackLabel(track: Track): string {
  const parts = [track.lang?.toUpperCase(), track.title].filter(Boolean);
  return parts.length > 0 ? parts.join(" · ") : `Track ${track.id}`;
}

const tracksOf = (kind: Track["kind"]) => (player()?.tracks ?? []).filter((t) => t.kind === kind);

/** The next id in the cycle; `withOff` adds an "off" step after the last track. */
export function nextTrackId(
  ids: number[],
  current: number | null,
  withOff: boolean,
): number | null {
  const cycle: (number | null)[] = withOff ? [...ids, null] : ids;
  if (cycle.length === 0) return null;
  const index = cycle.indexOf(current);
  return cycle[(index + 1) % cycle.length] ?? null;
}

async function setAudioDelay(ms: number): Promise<Feedback> {
  await audioSetDelay(ms);
  return { icon: Clock, label: "Audio delay", value: formatDelay(ms) };
}

async function setSubDelay(ms: number): Promise<Feedback> {
  await subSetDelay(ms);
  return { icon: Clock, label: "Subtitle delay", value: formatDelay(ms) };
}

const audioStep = () => config()?.controls.audio_delay_step_ms ?? 50;
const subStep = () => config()?.controls.sub_delay_step_ms ?? 50;

export const trackActions: Action[] = [
  {
    id: "audio.cycle_track",
    label: "Next audio track",
    group: "audio",
    icon: AudioLines,
    isEnabled: () => tracksOf("audio").length > 1,
    run: async () => {
      const tracks = tracksOf("audio");
      const id = nextTrackId(
        tracks.map((t) => t.id),
        player()?.selected.audio ?? null,
        false,
      );
      await tracksSelect("audio", id);
      const track = tracks.find((t) => t.id === id);
      return { icon: AudioLines, label: "Audio", value: track ? trackLabel(track) : "Off" };
    },
  },
  {
    id: "audio.delay_decrease",
    label: "Audio delay −",
    group: "audio",
    icon: Clock,
    repeatable: true,
    isEnabled: hasMedia,
    run: () => setAudioDelay((player()?.audioDelayMs ?? 0) - audioStep()),
  },
  {
    id: "audio.delay_increase",
    label: "Audio delay +",
    group: "audio",
    icon: Clock,
    repeatable: true,
    isEnabled: hasMedia,
    run: () => setAudioDelay((player()?.audioDelayMs ?? 0) + audioStep()),
  },
  {
    id: "audio.delay_reset",
    label: "Reset audio delay",
    group: "audio",
    icon: Clock,
    isEnabled: hasMedia,
    run: () => setAudioDelay(0),
  },
  {
    id: "audio.set_delay",
    label: "Set audio delay",
    group: "audio",
    icon: Clock,
    isEnabled: hasMedia,
    run: (ctx: RunContext) => setAudioDelay(Math.round(Number(ctx.args?.ms ?? 0))),
  },
  {
    id: "subtitles.cycle_track",
    label: "Next subtitle track",
    group: "subtitles",
    icon: Captions,
    isEnabled: () => tracksOf("sub").length > 0,
    run: async () => {
      const tracks = tracksOf("sub");
      const id = nextTrackId(
        tracks.map((t) => t.id),
        player()?.selected.sub ?? null,
        true,
      );
      await tracksSelect("sub", id);
      const track = tracks.find((t) => t.id === id);
      return { icon: Captions, label: "Subtitles", value: track ? trackLabel(track) : "Off" };
    },
  },
  {
    id: "subtitles.toggle_visibility",
    label: "Show / hide subtitles",
    group: "subtitles",
    icon: Eye,
    isEnabled: hasMedia,
    isOn: () => player()?.subVisible ?? true,
    run: async () => {
      const visible = !(player()?.subVisible ?? true);
      await subSetVisible(visible);
      return {
        icon: visible ? Eye : EyeOff,
        label: visible ? "Subtitles shown" : "Subtitles hidden",
      };
    },
  },
  {
    id: "subtitles.delay_decrease",
    label: "Subtitle delay −",
    group: "subtitles",
    icon: Clock,
    repeatable: true,
    isEnabled: hasMedia,
    run: () => setSubDelay((player()?.subDelayMs ?? 0) - subStep()),
  },
  {
    id: "subtitles.delay_increase",
    label: "Subtitle delay +",
    group: "subtitles",
    icon: Clock,
    repeatable: true,
    isEnabled: hasMedia,
    run: () => setSubDelay((player()?.subDelayMs ?? 0) + subStep()),
  },
  {
    id: "subtitles.delay_reset",
    label: "Reset subtitle delay",
    group: "subtitles",
    icon: Clock,
    isEnabled: hasMedia,
    run: () => setSubDelay(0),
  },
  {
    id: "subtitles.set_delay",
    label: "Set subtitle delay",
    group: "subtitles",
    icon: Clock,
    isEnabled: hasMedia,
    run: (ctx: RunContext) => setSubDelay(Math.round(Number(ctx.args?.ms ?? 0))),
  },
  {
    // The only path for raw mpv commands, and only from [[custom]] bindings (§3.4).
    id: "mpv.command",
    label: "Raw mpv command",
    group: "app",
    icon: Terminal,
    run: async (ctx: RunContext) => {
      const command = ctx.args?.command;
      if (!Array.isArray(command) || command.length === 0) return undefined;
      await playerMpvCommand(command.map(String));
      return undefined;
    },
  },
];
