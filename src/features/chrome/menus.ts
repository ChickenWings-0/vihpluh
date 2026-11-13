/**
 * The ☰ menu (DESIGN_SYSTEM §7.3) and the context menu (§8.8), both generated from the action
 * registry so they always agree with the keymap and with each other.
 */
import { AudioLines, Captions, Gauge, Ratio } from "lucide-react";
import { trackLabel } from "@/actions/definitions/tracks";
import { action, type MenuEntry, separator } from "@/components/Menu";
import { playerSetSpeed, tracksSelect } from "@/ipc/commands";
import { reportError } from "@/stores/osd";
import { usePlayerStore } from "@/stores/player";

const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 2];

function audioEntries(): MenuEntry[] {
  const state = usePlayerStore.getState().state;
  const tracks = (state?.tracks ?? []).filter((t) => t.kind === "audio");
  return tracks.map((t) => ({
    kind: "item",
    label: `${trackLabel(t)}${t.channels ? ` · ${t.channels}` : ""}`,
    hint: t.codec.toUpperCase(),
    checked: state?.selected.audio === t.id,
    onSelect: () => void tracksSelect("audio", t.id).catch(reportError),
  }));
}

function subtitleEntries(): MenuEntry[] {
  const state = usePlayerStore.getState().state;
  const tracks = (state?.tracks ?? []).filter((t) => t.kind === "sub");
  return [
    {
      kind: "item",
      label: "Off",
      checked: state?.selected.sub === null,
      onSelect: () => void tracksSelect("sub", null).catch(reportError),
    },
    ...tracks.map(
      (t): MenuEntry => ({
        kind: "item",
        label: trackLabel(t),
        hint: [t.forced ? "forced" : null, t.external ? "ext" : null].filter(Boolean).join(" "),
        checked: state?.selected.sub === t.id,
        onSelect: () => void tracksSelect("sub", t.id).catch(reportError),
      }),
    ),
    separator,
    action("subtitles.add_file"),
    action("subtitles.automatch"),
  ];
}

function speedEntries(): MenuEntry[] {
  const speed = usePlayerStore.getState().state?.speed ?? 1;
  return SPEEDS.map((value) => ({
    kind: "item",
    label: `${value}×`,
    checked: Math.abs(speed - value) < 0.001,
    onSelect: () => void playerSetSpeed(value).catch(reportError),
  }));
}

const hasMedia = () => Boolean(usePlayerStore.getState().state?.media);

/** Right-click on the video: essentials only, one screen, no scrolling (§8.8). */
export function contextMenuEntries(): MenuEntry[] {
  return [
    action("player.toggle_pause"),
    action("queue.next", "Next"),
    action("queue.previous", "Previous"),
    separator,
    { kind: "submenu", label: "Audio track", icon: AudioLines, entries: audioEntries() },
    {
      kind: "submenu",
      label: "Subtitles",
      icon: Captions,
      entries: hasMedia() ? subtitleEntries() : [],
    },
    { kind: "submenu", label: "Speed", icon: Gauge, entries: speedEntries() },
    {
      kind: "submenu",
      label: "Aspect ratio",
      icon: Ratio,
      entries: [action("video.cycle_aspect"), action("video.toggle_fill")],
    },
    separator,
    action("bookmarks.add", "Add bookmark"),
    action("player.screenshot", "Take screenshot"),
    separator,
    action("window.toggle_pip"),
    action("window.toggle_zen"),
    action("window.toggle_fullscreen"),
    separator,
    action("app.open_file"),
    action("app.reveal_current", "Show in file manager"),
    action("ui.open_settings"),
  ];
}

/** The single consolidated menu in the top bar (§7.3). */
export function appMenuEntries(): MenuEntry[] {
  return [
    action("app.open_file"),
    action("app.open_folder"),
    action("app.open_url"),
    action("app.paste_and_play"),
    separator,
    {
      kind: "submenu",
      label: "Playback",
      entries: [
        action("player.toggle_pause"),
        action("player.stop"),
        separator,
        { kind: "submenu", label: "Speed", icon: Gauge, entries: speedEntries() },
        action("player.speed_reset"),
        action("player.frame_step_forward"),
        action("player.frame_step_back"),
        action("player.ab_loop_cycle"),
        action("player.toggle_reverse"),
        separator,
        action("player.screenshot"),
        action("player.screenshot_clean"),
      ],
    },
    {
      kind: "submenu",
      label: "Audio",
      entries: [
        { kind: "submenu", label: "Track", icon: AudioLines, entries: audioEntries() },
        action("audio.cycle_track"),
        action("player.toggle_mute"),
        separator,
        action("audio.delay_increase"),
        action("audio.delay_decrease"),
        action("audio.delay_reset"),
      ],
    },
    {
      kind: "submenu",
      label: "Subtitles",
      entries: [
        {
          kind: "submenu",
          label: "Track",
          icon: Captions,
          entries: hasMedia() ? subtitleEntries() : [],
        },
        action("subtitles.cycle_track"),
        action("subtitles.toggle_visibility"),
        separator,
        action("subtitles.delay_increase"),
        action("subtitles.delay_decrease"),
        action("subtitles.delay_reset"),
      ],
    },
    {
      kind: "submenu",
      label: "Video",
      entries: [
        action("video.cycle_aspect"),
        action("video.toggle_fill"),
        action("video.rotate_cw"),
        action("video.zoom_in"),
        action("video.zoom_out"),
        action("video.reset_transform"),
      ],
    },
    separator,
    {
      kind: "submenu",
      label: "Presets",
      entries: [action("presets.save_current"), action("presets.update_current")],
    },
    action("ui.open_settings"),
    action("ui.show_shortcuts"),
    separator,
    action("app.quit"),
  ];
}
