/**
 * The current file's waveform for the selected audio track (ARCHITECTURE §11.2). Asks the
 * core when the seek-bar waveform is on (or `performance.waveform = "always"`), follows
 * `media://waveform` progress, and fetches the `.vwf` once ready. Streams and files without
 * audio have none.
 */
import { useEffect, useState } from "react";
import { waveformGet, waveformRequest } from "@/ipc/commands";
import { onWaveform } from "@/ipc/events";
import { parseVwf, type Waveform } from "@/lib/waveform";
import { useConfig } from "@/stores/config";
import { usePlayer } from "@/stores/player";

export interface WaveformState {
  wave: Waveform | null;
  /** 0–1 while the core is analysing; null otherwise. */
  progress: number | null;
}

const NONE: WaveformState = { wave: null, progress: null };

/** `force` asks for the waveform even when the seek-bar waveform is off (subtitle timeline). */
export function useWaveform(force = false): WaveformState {
  const key = usePlayer((s) => s.media?.key ?? "") ?? "";
  const isUrl = usePlayer((s) => s.media?.isUrl ?? false) ?? false;
  const track = usePlayer((s) => s.selected.audio) ?? null;
  const shown = useConfig((c) => c.interface.seek_waveform) ?? false;
  const mode = useConfig((c) => c.performance.waveform) ?? "on_demand";
  const [state, setState] = useState<WaveformState>(NONE);

  const wanted = mode !== "off" && (force || shown || mode === "always");
  const enabled = wanted && key !== "" && !isUrl && track !== null;

  useEffect(() => {
    setState(NONE);
    if (!enabled || track === null) return;
    let cancelled = false;
    const fetchWave = () =>
      void waveformGet(key, track)
        .then((buffer) => {
          if (!cancelled) setState({ wave: parseVwf(buffer), progress: null });
        })
        .catch(() => undefined);
    const unlisten = onWaveform((p) => {
      if (cancelled || p.mediaKey !== key || p.trackId !== track) return;
      if (p.status === "ready") fetchWave();
      else if (p.status === "running") setState({ wave: null, progress: p.progress });
      else setState(NONE);
    });
    void waveformRequest(key, track)
      .then(({ status }) => {
        if (cancelled) return;
        if (status === "ready") fetchWave();
        else if (status === "running" || status === "queued") setState({ wave: null, progress: 0 });
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
      unlisten();
    };
  }, [enabled, key, track]);

  return state;
}
