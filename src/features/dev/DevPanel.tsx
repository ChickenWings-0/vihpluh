/**
 * Temporary Phase 2 test UI (IMPLEMENTATION_PLAN §4): drives every player command and shows
 * the mirrored state. Replaced by the real chrome in Phase 3.
 */
import { type ReactNode, useRef, useState } from "react";
import type { Track } from "@/ipc/bindings/Track";
import type { TrackSlot } from "@/ipc/bindings/TrackSlot";
import {
  playerFrameStep,
  playerOpen,
  playerSeek,
  playerSetMuted,
  playerSetSpeed,
  playerSetVolume,
  playerStop,
  playerTogglePause,
  tracksSelect,
} from "@/ipc/commands";
import { formatTime } from "@/lib/time";
import { useAnimationFrame } from "@/lib/useAnimationFrame";
import { useConfig } from "@/stores/config";
import { usePlayer, usePlayerStore } from "@/stores/player";
import { getDuration, getPosition } from "@/stores/time";

const report = (error: unknown) => console.error(error);

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <span className="w-16 shrink-0 text-fg-3">{label}</span>
      {children}
    </div>
  );
}

function Button({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="h-(--control-sm) rounded-control border border-line bg-raised px-2 text-fg hover:bg-hover"
    >
      {children}
    </button>
  );
}

function SeekRow() {
  const slider = useRef<HTMLInputElement>(null);
  const label = useRef<HTMLSpanElement>(null);
  const dragging = useRef(false);
  const lastSeek = useRef(0);

  useAnimationFrame(() => {
    const pos = getPosition();
    const dur = getDuration();
    if (slider.current && !dragging.current) {
      slider.current.max = String(dur || 0);
      slider.current.value = String(pos);
    }
    if (label.current) label.current.textContent = `${formatTime(pos)} / ${formatTime(dur)}`;
  });

  const seek = (value: number, exact: boolean) =>
    playerSeek({ kind: "absolute", value, exact }).catch(report);

  return (
    <Row label="position">
      <input
        ref={slider}
        type="range"
        min={0}
        step={0.1}
        className="w-80 accent-accent"
        aria-label="Position"
        onPointerDown={() => {
          dragging.current = true;
        }}
        onInput={(e) => {
          // Keyframe seeks while dragging, throttled (DESIGN_SYSTEM §8.1).
          const now = performance.now();
          if (now - lastSeek.current >= 50) {
            lastSeek.current = now;
            void seek(Number(e.currentTarget.value), false);
          }
        }}
        onPointerUp={(e) => {
          dragging.current = false;
          void seek(Number(e.currentTarget.value), true);
        }}
      />
      <span ref={label} className="tabular font-mono" />
    </Row>
  );
}

function TrackRow({
  label,
  slot,
  tracks,
  selected,
}: {
  label: string;
  slot: TrackSlot;
  tracks: Track[];
  selected: number | null | undefined;
}) {
  return (
    <Row label={label}>
      <select
        className="h-(--control-sm) max-w-96 rounded-control border border-line bg-raised px-1 text-fg"
        aria-label={label}
        value={selected ?? ""}
        onChange={(e) => {
          const id = e.currentTarget.value === "" ? null : Number(e.currentTarget.value);
          tracksSelect(slot, id).catch(report);
        }}
      >
        <option value="">Off</option>
        {tracks.map((t) => (
          <option key={t.id} value={t.id}>
            {[t.id, t.lang, t.title, t.codec, t.channels, t.external ? "external" : null]
              .filter(Boolean)
              .join(" · ")}
          </option>
        ))}
      </select>
    </Row>
  );
}

export function DevPanel() {
  const [path, setPath] = useState("");
  const [showDump, setShowDump] = useState(false);
  const status = usePlayer((s) => s.status);
  const title = usePlayer((s) => s.media?.title ?? null);
  const volume = usePlayer((s) => s.volume);
  const muted = usePlayer((s) => s.muted);
  const speed = usePlayer((s) => s.speed);
  const tracks = usePlayer((s) => s.tracks) ?? [];
  const selected = usePlayer((s) => s.selected);
  const error = usePlayer((s) => s.error);
  const maxVolume = useConfig((c) => c.audio.max_volume) ?? 130;
  const state = usePlayerStore((s) => (showDump ? s.state : null));

  const open = () => {
    const source = path.trim().replace(/^"|"$/g, "");
    if (source) playerOpen({ sources: [source], mode: "replace" }).catch(report);
  };

  return (
    <div className="absolute bottom-2 left-2 z-(--z-chrome) flex max-h-[calc(100%-1rem)] w-[36rem] flex-col gap-1.5 overflow-auto rounded-surface border border-line-strong bg-surface/90 p-3 text-sm">
      <Row label="open">
        <input
          className="h-(--control-sm) flex-1 rounded-control border border-line bg-raised px-2 text-fg"
          placeholder="Path or URL"
          aria-label="Path or URL"
          value={path}
          onChange={(e) => setPath(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") open();
          }}
        />
        <Button onClick={open}>Open</Button>
      </Row>
      <Row label="status">
        <span className="font-mono">{status ?? "…"}</span>
        <span className="truncate text-fg-2">{title}</span>
        {error && <span className="text-danger">{error}</span>}
      </Row>
      <Row label="transport">
        <Button onClick={() => playerTogglePause().catch(report)}>Play / Pause</Button>
        <Button onClick={() => playerStop().catch(report)}>Stop</Button>
        <Button onClick={() => playerFrameStep("backward").catch(report)}>Frame −</Button>
        <Button onClick={() => playerFrameStep("forward").catch(report)}>Frame +</Button>
      </Row>
      <SeekRow />
      <Row label="volume">
        <input
          type="range"
          min={0}
          max={maxVolume}
          value={volume ?? 100}
          className="w-48 accent-accent"
          aria-label="Volume"
          onChange={(e) => playerSetVolume(Number(e.currentTarget.value)).catch(report)}
        />
        <span className="tabular w-10 font-mono">{Math.round(volume ?? 0)}</span>
        <Button onClick={() => playerSetMuted(!muted).catch(report)}>
          {muted ? "Unmute" : "Mute"}
        </Button>
      </Row>
      <Row label="speed">
        {[0.5, 1, 1.5, 2].map((value) => (
          <Button key={value} onClick={() => playerSetSpeed(value).catch(report)}>
            {value}×
          </Button>
        ))}
        <span className="tabular font-mono">{speed?.toFixed(2)}×</span>
      </Row>
      <TrackRow
        label="video"
        slot="video"
        tracks={tracks.filter((t) => t.kind === "video")}
        selected={selected?.video}
      />
      <TrackRow
        label="audio"
        slot="audio"
        tracks={tracks.filter((t) => t.kind === "audio")}
        selected={selected?.audio}
      />
      <TrackRow
        label="subtitles"
        slot="sub"
        tracks={tracks.filter((t) => t.kind === "sub")}
        selected={selected?.sub}
      />
      <Row label="state">
        <Button onClick={() => setShowDump((v) => !v)}>{showDump ? "Hide" : "Show"} dump</Button>
      </Row>
      {state && (
        <pre className="selectable max-h-64 overflow-auto rounded-control bg-raised p-2 font-mono text-xs">
          {JSON.stringify(state, null, 1)}
        </pre>
      )}
    </div>
  );
}
