/** Typed wrapper per core command (ARCHITECTURE §7.3). Components never call `invoke` directly. */
import type { AbLoop } from "./bindings/AbLoop";
import type { AppInfo } from "./bindings/AppInfo";
import type { Config } from "./bindings/Config";
import type { Direction } from "./bindings/Direction";
import type { OpenRequest } from "./bindings/OpenRequest";
import type { PlayerState } from "./bindings/PlayerState";
import type { SeekRequest } from "./bindings/SeekRequest";
import type { TrackSlot } from "./bindings/TrackSlot";
import { invoke } from "./transport";

// ── app ──────────────────────────────────────────────────────────────────────
export const appInfo = () => invoke<AppInfo>("app_info");
export const appReady = () => invoke<void>("app_ready");
export const appQuit = () => invoke<void>("app_quit");

// ── player ───────────────────────────────────────────────────────────────────
export const playerSnapshot = () => invoke<PlayerState>("player_snapshot");
export const playerOpen = (req: OpenRequest) => invoke<void>("player_open", { req });
export const playerSetPaused = (paused: boolean) => invoke<void>("player_set_paused", { paused });
export const playerTogglePause = () => invoke<void>("player_toggle_pause");
export const playerSeek = (req: SeekRequest) => invoke<void>("player_seek", { req });
export const playerFrameStep = (direction: Direction) =>
  invoke<void>("player_frame_step", { direction });
export const playerSetSpeed = (value: number) => invoke<void>("player_set_speed", { value });
export const playerSetVolume = (value: number) => invoke<void>("player_set_volume", { value });
export const playerSetMuted = (muted: boolean) => invoke<void>("player_set_muted", { muted });
export const playerSetDirection = (direction: Direction) =>
  invoke<void>("player_set_direction", { direction });
export const playerSetLoopFile = (enabled: boolean) =>
  invoke<void>("player_set_loop_file", { enabled });
export const playerSetAbLoop = (req: AbLoop) => invoke<void>("player_set_ab_loop", { req });
export const playerStop = () => invoke<void>("player_stop");
export const playerScreenshot = (subtitles: boolean) =>
  invoke<string>("player_screenshot", { subtitles });
/** Backs the `mpv.command` action only. */
export const playerMpvCommand = (args: string[]) => invoke<void>("player_mpv_command", { args });

// ── tracks ───────────────────────────────────────────────────────────────────
export const tracksSelect = (kind: TrackSlot, id: number | null) =>
  invoke<void>("tracks_select", { kind, id });

// ── config ───────────────────────────────────────────────────────────────────
export const configGet = () => invoke<Config>("config_get");
export const configSet = (path: string, value: unknown) =>
  invoke<void>("config_set", { path, value });
export const configReset = (path?: string) => invoke<void>("config_reset", { path });
