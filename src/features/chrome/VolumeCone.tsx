/**
 * Cone volume slider (DESIGN_SYSTEM §8.2): a right-triangle wedge that fills as volume rises.
 * 0 – audio.max_volume; a 1 px notch marks 100 %; the part above 100 % fills in --danger.
 * Muted keeps the level but greys the fill. Click/drag sets, wheel steps, double-click
 * resets to 100 %, ←/→ when focused. The mute button sits to its left.
 */
import { Volume1, Volume2, VolumeX } from "lucide-react";
import { type KeyboardEvent, type PointerEvent, useRef, useState, type WheelEvent } from "react";
import { IconButton } from "@/components/IconButton";
import { playerSetMuted, playerSetVolume } from "@/ipc/commands";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { useUiStore } from "@/stores/ui";

const W = 72;
const H = 20;
const SEND_MS = 30;

export function volumeIcon(volume: number, muted: boolean) {
  return muted ? VolumeX : volume < 50 ? Volume1 : Volume2;
}

export function MuteButton() {
  const volume = usePlayer((s) => s.volume) ?? 100;
  const muted = usePlayer((s) => s.muted) ?? false;
  return (
    <IconButton
      icon={volumeIcon(volume, muted)}
      label={muted ? "Unmute" : "Mute"}
      actionId="player.toggle_mute"
      on={false}
    />
  );
}

export function VolumeCone() {
  const volume = usePlayer((s) => s.volume) ?? 100;
  const muted = usePlayer((s) => s.muted) ?? false;
  const max = useConfig((c) => c.audio.max_volume) ?? 130;
  const step = useConfig((c) => c.controls.volume_step) ?? 5;
  const hold = useUiStore((s) => s.hold);
  const area = useRef<HTMLDivElement>(null);
  const lastSend = useRef(0);
  // While dragging, show the pointer's value at once; the core's value follows.
  const [local, setLocal] = useState<number | null>(null);
  const shown = local ?? volume;

  const set = (v: number) => {
    const value = Math.round(Math.min(max, Math.max(0, v)));
    void playerSetVolume(value).catch(reportError);
    if (muted) void playerSetMuted(false).catch(reportError);
    return value;
  };

  const valueAt = (clientX: number) => {
    const rect = area.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return volume;
    return Math.min(max, Math.max(0, ((clientX - rect.left) / rect.width) * max));
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    hold("drag", true);
    setLocal(set(valueAt(e.clientX)));
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
    const v = Math.round(valueAt(e.clientX));
    setLocal(v);
    const now = performance.now();
    if (now - lastSend.current >= SEND_MS) {
      lastSend.current = now;
      set(v);
    }
  };
  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
    e.currentTarget.releasePointerCapture(e.pointerId);
    hold("drag", false);
    set(valueAt(e.clientX));
    setLocal(null);
  };
  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    if (Math.abs(e.deltaY) < 1) return;
    e.stopPropagation();
    set(volume + (e.deltaY < 0 ? step : -step));
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const delta =
      e.key === "ArrowRight" || e.key === "ArrowUp"
        ? step
        : e.key === "ArrowLeft" || e.key === "ArrowDown"
          ? -step
          : null;
    if (delta === null) return;
    e.preventDefault();
    e.stopPropagation();
    set(volume + delta);
  };

  const fillX = (Math.min(shown, max) / max) * W;
  const hundredX = (Math.min(100, max) / max) * W;
  const normalFill = muted ? "var(--text-disabled)" : "var(--accent)";
  const boostFill = muted ? "var(--text-disabled)" : "var(--danger)";

  return (
    <div
      ref={area}
      role="slider"
      tabIndex={0}
      aria-label="Volume"
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={Math.round(shown)}
      aria-valuetext={`Volume ${Math.round(shown)} percent${muted ? ", muted" : ""}`}
      title={`Volume ${Math.round(shown)} %`}
      onPointerDown={onPointerDown}
      // Pointer use never moves focus to a control (DESIGN_SYSTEM §13); Tab still does.
      onMouseDown={(e) => e.preventDefault()}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onWheel={onWheel}
      onKeyDown={onKeyDown}
      onDoubleClick={() => set(100)}
      className="flex h-8 w-18 shrink-0 cursor-pointer touch-none items-center rounded-control"
    >
      <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
        <defs>
          <clipPath id="vihpluh-cone">
            <polygon points={`0,${H} ${W},${H} ${W},0`} />
          </clipPath>
        </defs>
        <g clipPath="url(#vihpluh-cone)">
          <rect x={0} y={0} width={W} height={H} fill="var(--track)" />
          <rect x={0} y={0} width={Math.min(fillX, hundredX)} height={H} fill={normalFill} />
          {fillX > hundredX && (
            <rect x={hundredX} y={0} width={fillX - hundredX} height={H} fill={boostFill} />
          )}
          {max > 100 && <rect x={hundredX} y={0} width={1} height={H} fill="var(--bg-surface)" />}
        </g>
      </svg>
    </div>
  );
}
