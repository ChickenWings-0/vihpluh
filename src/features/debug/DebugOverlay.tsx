import { useEffect, useState } from "react";
import type { AppInfo } from "@/ipc/bindings/AppInfo";
import type { DebugStats } from "@/ipc/bindings/DebugStats";
import { onDebugStats } from "@/ipc/events";

/**
 * Development-only readout of the decode path plus compositing probes for spike S1
 * (IMPLEMENTATION_PLAN §3.2–§3.3): a half-transparent box, an animated element, and the last
 * pointer event, all drawn over the video.
 */
export function DebugOverlay({ info }: { info: AppInfo | null }) {
  const [stats, setStats] = useState<DebugStats | null>(null);
  const [pointer, setPointer] = useState({ x: 0, y: 0, moves: 0, clicks: 0 });
  useEffect(() => onDebugStats(setStats), []);

  useEffect(() => {
    const onMove = (e: PointerEvent) =>
      setPointer((p) => ({ ...p, x: e.clientX, y: e.clientY, moves: p.moves + 1 }));
    const onClick = () => setPointer((p) => ({ ...p, clicks: p.clicks + 1 }));
    window.addEventListener("pointermove", onMove);
    window.addEventListener("click", onClick);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("click", onClick);
    };
  }, []);

  const caps = info?.capabilities;
  const rows: [string, string][] = [
    ["backend", caps ? `${caps.backend} · ${caps.renderer} · ${caps.session}` : "…"],
    ["gpu", caps?.gpuVendor ?? "…"],
    ["hwdec", stats?.hwdec || "—"],
    ["vo", stats?.vo || "—"],
    ["video", stats?.videoCodec ? `${stats.videoCodec} ${stats.width}×${stats.height}` : "—"],
    ["fps", stats?.fps ? stats.fps.toFixed(3) : "—"],
    ["dropped", stats ? `${stats.droppedFrames} vo · ${stats.decoderDroppedFrames} dec` : "—"],
    ["pointer", `${pointer.x},${pointer.y} · ${pointer.moves} moves · ${pointer.clicks} clicks`],
    ["window", `${window.innerWidth}×${window.innerHeight} @${window.devicePixelRatio}`],
    ["mpv", info?.mpvVersion ?? "…"],
  ];

  return (
    <>
      <div className="pointer-events-none absolute top-2 left-2 z-(--z-osd) rounded-surface border border-line-strong bg-surface/90 px-3 py-2 font-mono text-fg text-sm">
        {rows.map(([label, value]) => (
          <div key={label} className="flex gap-3">
            <span className="w-16 text-fg-3">{label}</span>
            <span className="tabular">{value}</span>
          </div>
        ))}
      </div>
      <div className="pointer-events-none absolute right-4 bottom-4 z-(--z-osd) flex items-end gap-4">
        <div className="h-24 w-40 rounded-surface bg-accent/50 p-2 text-on-accent text-sm">
          50 % alpha
        </div>
        <div className="h-12 w-12 animate-spin rounded-control bg-fg" />
      </div>
    </>
  );
}
