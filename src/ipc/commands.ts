/** Typed wrapper per core command (ARCHITECTURE §7.3). Components never call `invoke` directly. */
import type { AbLoop } from "./bindings/AbLoop";
import type { AppInfo } from "./bindings/AppInfo";
import type { CacheInfo } from "./bindings/CacheInfo";
import type { Config } from "./bindings/Config";
import type { Direction } from "./bindings/Direction";
import type { EqPreset } from "./bindings/EqPreset";
import type { EqState } from "./bindings/EqState";
import type { Margins } from "./bindings/Margins";
import type { OpenRequest } from "./bindings/OpenRequest";
import type { PickKind } from "./bindings/PickKind";
import type { PlayerState } from "./bindings/PlayerState";
import type { QueueState } from "./bindings/QueueState";
import type { RecentEntry } from "./bindings/RecentEntry";
import type { Repeat } from "./bindings/Repeat";
import type { ResolvedKeymap } from "./bindings/ResolvedKeymap";
import type { SeekRequest } from "./bindings/SeekRequest";
import type { ShaderEntry } from "./bindings/ShaderEntry";
import type { SleepRequest } from "./bindings/SleepRequest";
import type { TrackSlot } from "./bindings/TrackSlot";
import type { UiPatch } from "./bindings/UiPatch";
import type { UiState } from "./bindings/UiState";
import type { VideoPatch } from "./bindings/VideoPatch";
import type { WaveformRequestResult } from "./bindings/WaveformRequestResult";
import type { WindowMode } from "./bindings/WindowMode";
import type { WindowState } from "./bindings/WindowState";
import { invoke } from "./transport";

// ── app ──────────────────────────────────────────────────────────────────────
export const appInfo = () => invoke<AppInfo>("app_info");
export const appReady = () => invoke<void>("app_ready");
export const appQuit = () => invoke<void>("app_quit");
/** Shows a file selected in the system file manager. */
export const appReveal = (path: string) => invoke<void>("app_reveal", { path });
/** Native open dialog; resolves to `[]` when cancelled. */
export const appPickMedia = (kind: PickKind) => invoke<string[]>("app_pick_media", { kind });

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

// ── audio / subtitles ────────────────────────────────────────────────────────
export const audioSetDelay = (ms: number) => invoke<void>("audio_set_delay", { ms });
export const audioSetDevice = (name: string) => invoke<void>("audio_set_device", { name });
/** Live EQ; the on/off switch and preset are also saved as defaults. */
export const audioSetEq = (eq: EqState) => invoke<void>("audio_set_eq", { eq });
/** Writes `audio.normalize`; the core follows the setting. */
export const audioSetNormalize = (enabled: boolean) =>
  invoke<void>("audio_set_normalize", { enabled });
export const eqPresetsList = () => invoke<EqPreset[]>("eq_presets_list");
export const eqPresetSave = (preset: EqPreset) => invoke<void>("eq_preset_save", { preset });
export const eqPresetDelete = (id: string) => invoke<void>("eq_preset_delete", { id });
export const subSetDelay = (ms: number) => invoke<void>("sub_set_delay", { ms });
export const subSetVisible = (visible: boolean) => invoke<void>("sub_set_visible", { visible });

// ── queue ────────────────────────────────────────────────────────────────────
export const queueSnapshot = () => invoke<QueueState>("queue_snapshot");
export const queueAdd = (sources: string[], at?: number) =>
  invoke<void>("queue_add", { sources, at: at ?? null });
export const queueRemove = (ids: string[]) => invoke<void>("queue_remove", { ids });
export const queueMove = (ids: string[], toIndex: number) =>
  invoke<void>("queue_move", { ids, toIndex });
export const queueClear = (keepCurrent: boolean) => invoke<void>("queue_clear", { keepCurrent });
export const queuePlay = (id: string) => invoke<void>("queue_play", { id });
export const queueNext = () => invoke<void>("queue_next");
export const queuePrev = () => invoke<void>("queue_prev");
export const queueSetRepeat = (mode: Repeat) => invoke<void>("queue_set_repeat", { mode });
export const queueSetShuffle = (enabled: boolean) => invoke<void>("queue_set_shuffle", { enabled });
export const queueSetStopAfterCurrent = (enabled: boolean) =>
  invoke<void>("queue_set_stop_after_current", { enabled });

// ── library / session ────────────────────────────────────────────────────────
export const libraryRecent = (limit: number) => invoke<RecentEntry[]>("library_recent", { limit });
export const sessionUiGet = () => invoke<UiState>("session_ui_get");
export const sessionUiSet = (patch: UiPatch) => invoke<void>("session_ui_set", { patch });

// ── video ────────────────────────────────────────────────────────────────────
export const videoSetMargins = (req: Margins) => invoke<void>("video_set_margins", { req });
export const videoSetAdjust = (patch: VideoPatch) => invoke<void>("video_set_adjust", { patch });
export const videoReset = () => invoke<void>("video_reset");
/** `"pq"`/`"hlg"` while HDR passes through to the display; null when not reported. */
export const videoOutputTransfer = () => invoke<string | null>("video_output_transfer");
export const shadersList = () => invoke<ShaderEntry[]>("shaders_list");
export const playerSetSleepTimer = (req: SleepRequest) =>
  invoke<void>("player_set_sleep_timer", { req });
/** The stream inspector sends `player://inspector` at 2 Hz only while subscribed. */
export const inspectorSubscribe = (enabled: boolean) =>
  invoke<void>("inspector_subscribe", { enabled });

// ── keybinds ─────────────────────────────────────────────────────────────────
export const keybindsGet = () => invoke<ResolvedKeymap>("keybinds_get");

// ── window ───────────────────────────────────────────────────────────────────
export const windowGetState = () => invoke<WindowState>("window_get_state");
export const windowSetMode = (mode: WindowMode) => invoke<void>("window_set_mode", { mode });
export const windowSetAlwaysOnTop = (enabled: boolean) =>
  invoke<void>("window_set_always_on_top", { enabled });
export const windowMinimize = () => invoke<void>("window_minimize");
export const windowToggleMaximize = () => invoke<void>("window_toggle_maximize");
export const windowClose = () => invoke<void>("window_close");

// ── config ───────────────────────────────────────────────────────────────────
export const configGet = () => invoke<Config>("config_get");
export const configSet = (path: string, value: unknown) =>
  invoke<void>("config_set", { path, value });
export const configReset = (path?: string) => invoke<void>("config_reset", { path });

// ── media services (ARCHITECTURE §11) ────────────────────────────────────────
/** Starts or reports the waveform for an audio track of the current file. */
export const waveformRequest = (mediaKey: string, trackId: number) =>
  invoke<WaveformRequestResult>("waveform_request", { mediaKey, trackId });
/** The finished `.vwf` bytes (§11.2). */
export const waveformGet = (mediaKey: string, trackId: number) =>
  invoke<ArrayBuffer>("waveform_get", { mediaKey, trackId });
export const cacheInfo = () => invoke<CacheInfo>("cache_info");
export const cacheClear = () => invoke<void>("cache_clear");
