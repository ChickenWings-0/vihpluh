/**
 * Stream info (DESIGN_SYSTEM §10.4): selectable mono values, refreshed at 2 Hz only while
 * mounted *and* the page is visible — the core sends nothing otherwise.
 */
import { Copy, FolderOpen } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/Button";
import { Section } from "@/components/Controls";
import type { InspectorStats } from "@/ipc/bindings/InspectorStats";
import { appReveal, inspectorSubscribe } from "@/ipc/commands";
import { onInspector } from "@/ipc/events";
import { reportError, useOsdStore } from "@/stores/osd";

const mbps = (bits: number | null) => (bits ? `${(bits / 1e6).toFixed(1)} Mb/s` : null);
const kbps = (bits: number | null) => (bits ? `${Math.round(bits / 1000)} kb/s` : null);
const join = (...parts: (string | null | undefined | false)[]) => parts.filter(Boolean).join(" · ");

function hdrLabel(primaries: string | null, transfer: string | null): string | null {
  if (transfer === "pq") return `HDR10 (${primaries ?? "?"}/PQ)`;
  if (transfer === "hlg") return `HLG (${primaries ?? "?"})`;
  return null;
}

function size(bytes: number | null): string | null {
  if (bytes === null) return null;
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GiB`;
  return `${Math.round(bytes / 1024 ** 2)} MiB`;
}

export function rows(s: InspectorStats): [string, string][] {
  const out: [string, string][] = [];
  const video = join(
    s.videoCodec?.toUpperCase(),
    s.width && s.height ? `${s.width}×${s.height}` : null,
    s.fps ? `${s.fps.toFixed(3).replace(/\.?0+$/, "")} fps` : null,
    s.bitDepth ? `${s.bitDepth}-bit` : null,
    hdrLabel(s.primaries, s.transfer),
    mbps(s.videoBitrate),
  );
  if (video) out.push(["Video", video]);
  const decoder = join(
    s.hwdec && s.hwdec !== "no" ? `${s.hwdec} (hardware)` : s.videoCodec ? "software" : null,
    s.vo,
  );
  if (decoder) {
    out.push([
      "Decoder",
      `${decoder} · dropped ${s.droppedFrames + s.decoderDroppedFrames} · delayed ${s.delayedFrames}`,
    ]);
  }
  const audio = join(
    s.audioCodec?.toUpperCase(),
    s.channels,
    s.sampleRate ? `${s.sampleRate / 1000} kHz` : null,
    kbps(s.audioBitrate),
  );
  if (audio) out.push(["Audio", `${audio}${s.audioOut && s.ao ? ` → ${s.ao} ${s.audioOut}` : ""}`]);
  if (s.subCodec)
    out.push(["Subs", join(s.subCodec.toUpperCase(), s.subLang, s.subTitle && `"${s.subTitle}"`)]);
  const file = join(s.container, size(s.fileSize), s.path);
  if (file) out.push(["File", file]);
  if (s.cacheSeconds !== null) {
    out.push(["Cache", join(`${Math.round(s.cacheSeconds)} s ahead`, size(s.cacheBytes))]);
  }
  return out;
}

export function Inspector({ scrollRequest }: { scrollRequest: number }) {
  const [stats, setStats] = useState<InspectorStats | null>(null);
  const [visible, setVisible] = useState(document.visibilityState === "visible");
  const [section, setSection] = useState<HTMLDivElement | null>(null);

  useEffect(() => {
    const onVisibility = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, []);

  useEffect(() => {
    if (!visible) return;
    const unlisten = onInspector(setStats);
    void inspectorSubscribe(true).catch(reportError);
    return () => {
      unlisten();
      void inspectorSubscribe(false).catch(() => undefined);
    };
  }, [visible]);

  useEffect(() => {
    if (scrollRequest > 0) section?.scrollIntoView({ block: "start" });
  }, [scrollRequest, section]);

  const lines = stats ? rows(stats) : [];
  const copyAll = () => {
    const text = lines.map(([k, v]) => `${k}: ${v}`).join("\n");
    void navigator.clipboard
      .writeText(text)
      .then(() => useOsdStore.getState().pushToast({ kind: "success", text: "Stream info copied" }))
      .catch(reportError);
  };

  return (
    <div ref={setSection}>
      <Section
        title="Stream info"
        right={
          <Button
            variant="ghost"
            size="sm"
            icon={Copy}
            disabled={lines.length === 0}
            onClick={copyAll}
          >
            Copy all
          </Button>
        }
      >
        {lines.length === 0 ? (
          <p className="text-fg-3 text-sm">Nothing is playing.</p>
        ) : (
          <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 select-text">
            {lines.map(([label, value]) => (
              <div key={label} className="contents">
                <dt className="text-fg-2 text-sm">{label}</dt>
                <dd className="min-w-0 break-words font-mono text-fg text-sm">{value}</dd>
              </div>
            ))}
          </dl>
        )}
        {stats?.path && (
          <div className="flex justify-end">
            <Button
              variant="ghost"
              size="sm"
              icon={FolderOpen}
              onClick={() => stats.path && void appReveal(stats.path).catch(reportError)}
            >
              Show in folder
            </Button>
          </div>
        )}
      </Section>
    </div>
  );
}
