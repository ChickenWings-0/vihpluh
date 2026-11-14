/**
 * Actions that need arguments (KEYBINDS_AND_CONFIG §3.4): they are bound with `[[custom]]`
 * in `keybinds.toml` and run by buttons and menus that supply the value, so they are listed
 * neither in the hotkey editor nor in the command palette.
 */
export const PARAMETERISED: readonly string[] = [
  "player.seek_relative",
  "player.seek_absolute",
  "player.set_speed",
  "player.set_volume",
  "audio.set_delay",
  "subtitles.set_delay",
  "audio.set_equalizer_preset",
  "video.set_aspect",
  "presets.apply",
  "player.set_sleep_timer",
  "mpv.command",
];
