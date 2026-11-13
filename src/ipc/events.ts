/** Typed subscriptions to core events (ARCHITECTURE §7.4). */
import type { ConfigChanged } from "./bindings/ConfigChanged";
import type { CuesEvent } from "./bindings/CuesEvent";
import type { DebugStats } from "./bindings/DebugStats";
import type { InspectorStats } from "./bindings/InspectorStats";
import type { OpenEvent } from "./bindings/OpenEvent";
import type { OsdEvent } from "./bindings/OsdEvent";
import type { PlayerState } from "./bindings/PlayerState";
import type { QueueState } from "./bindings/QueueState";
import type { ResolvedKeymap } from "./bindings/ResolvedKeymap";
import type { TimeEvent } from "./bindings/TimeEvent";
import type { WaveformProgress } from "./bindings/WaveformProgress";
import type { WindowState } from "./bindings/WindowState";
import { listen, type Unlisten } from "./transport";

/** A shallow patch: each present key replaces the old value wholesale. */
export type PlayerPatch = { rev: number } & Partial<Omit<PlayerState, "rev">>;

export const onPlayerPatch = (handler: (patch: PlayerPatch) => void): Unlisten =>
  listen<PlayerPatch>("player://patch", handler);

export const onPlayerTime = (handler: (time: TimeEvent) => void): Unlisten =>
  listen<TimeEvent>("player://time", handler);

export const onConfigChanged = (handler: (change: ConfigChanged) => void): Unlisten =>
  listen<ConfigChanged>("config://changed", handler);

export const onOsd = (handler: (osd: OsdEvent) => void): Unlisten =>
  listen<OsdEvent>("osd://show", handler);

export const onAppOpen = (handler: (open: OpenEvent) => void): Unlisten =>
  listen<OpenEvent>("app://open", handler);

export const onDebugStats = (handler: (stats: DebugStats) => void): Unlisten =>
  listen<DebugStats>("debug://stats", handler);

export const onQueueChanged = (handler: (queue: QueueState) => void): Unlisten =>
  listen<QueueState>("queue://changed", handler);

export const onKeybindsChanged = (handler: (keymap: ResolvedKeymap) => void): Unlisten =>
  listen<ResolvedKeymap>("keybinds://changed", handler);

export const onWindowState = (handler: (state: WindowState) => void): Unlisten =>
  listen<WindowState>("window://state", handler);

export const onInspector = (handler: (stats: InspectorStats) => void): Unlisten =>
  listen<InspectorStats>("player://inspector", handler);

export const onCues = (handler: (event: CuesEvent) => void): Unlisten =>
  listen<CuesEvent>("media://cues", handler);

export const onWaveform = (handler: (progress: WaveformProgress) => void): Unlisten =>
  listen<WaveformProgress>("media://waveform", handler);
