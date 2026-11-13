/**
 * Video tab (DESIGN_SYSTEM §10.3): picture sliders, aspect, rotate/flip, zoom, tone mapping,
 * enhancements, and user shaders. Everything previews live on the running picture. Options
 * the renderer cannot do are left out (`capabilities.renderer`), not shown disabled.
 */
import { FlipHorizontal2, FlipVertical2, FolderOpen, RotateCcw, RotateCw } from "lucide-react";
import { useEffect, useState } from "react";
import { dispatch } from "@/actions";
import { ASPECTS, toneCurves, zoomLabel } from "@/actions/definitions/media";
import { Button } from "@/components/Button";
import { Chip, Section, Segmented, Select, Toggle } from "@/components/Controls";
import { ParamRow } from "@/components/ParamRow";
import type { ShaderEntry } from "@/ipc/bindings/ShaderEntry";
import type { VideoPatch } from "@/ipc/bindings/VideoPatch";
import {
  appReveal,
  shadersList,
  videoOutputTransfer,
  videoReset,
  videoSetAdjust,
} from "@/ipc/commands";
import { useAppStore } from "@/stores/app";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";

const set = (patch: VideoPatch) => void videoSetAdjust(patch).catch(reportError);
const signed = (v: number) => `${v > 0 ? "+" : v < 0 ? "−" : ""}${Math.abs(Math.round(v))}`;

const PICTURE = [
  ["brightness", "Brightness"],
  ["contrast", "Contrast"],
  ["saturation", "Saturation"],
  ["gamma", "Gamma"],
  ["hue", "Hue"],
] as const;

function HdrStatus() {
  const capable = useAppStore((s) => s.info?.capabilities.hdrPassthrough) ?? false;
  const wanted = useConfig((c) => c.video.hdr_passthrough) !== "off";
  const hdrContent = usePlayer((s) =>
    s.tracks.some((t) => t.kind === "video" && t.id === s.selected.video && Boolean(t.hdr)),
  );
  const [transfer, setTransfer] = useState<string | null>(null);
  useEffect(() => {
    if (!hdrContent) return;
    let cancelled = false;
    const poll = () =>
      void videoOutputTransfer()
        .then((t) => !cancelled && setTransfer(t))
        .catch(() => undefined);
    poll();
    const timer = setInterval(poll, 2000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [hdrContent]);

  let text: string;
  let active = false;
  if (!capable) text = "Passthrough unavailable";
  else if (!wanted) text = "Passthrough off";
  else if (!hdrContent) text = "SDR video";
  else if (transfer === "pq" || transfer === "hlg") {
    text = "Passthrough active";
    active = true;
  } else text = "Tone-mapped";
  return <Chip accent={active}>{text}</Chip>;
}

function Shaders({ active }: { active: string[] }) {
  const [shaders, setShaders] = useState<ShaderEntry[]>([]);
  const configDir = useAppStore((s) => s.info?.paths.config);
  useEffect(() => {
    void shadersList().then(setShaders).catch(reportError);
  }, []);
  const toggle = (path: string, on: boolean) =>
    set({ shaders: on ? [...active, path] : active.filter((p) => p !== path) });
  return (
    <Section
      title="Shaders"
      right={
        configDir ? (
          <Button
            variant="ghost"
            size="sm"
            icon={FolderOpen}
            onClick={() =>
              void appReveal(`${configDir}/shaders`)
                .then(() => shadersList().then(setShaders))
                .catch(reportError)
            }
          >
            Open folder
          </Button>
        ) : null
      }
    >
      {shaders.length === 0 ? (
        <p className="text-fg-3 text-sm">
          Put .glsl shader files in the shaders folder to list them here.
        </p>
      ) : (
        <ul className="flex flex-col gap-1">
          {shaders.map((shader) => {
            const on = active.includes(shader.path);
            return (
              <li key={shader.path}>
                <label className="flex h-7 cursor-pointer items-center gap-2 rounded-control px-1 text-base text-fg hover:bg-hover">
                  <input
                    type="checkbox"
                    checked={on}
                    onChange={(e) => toggle(shader.path, e.target.checked)}
                    className="h-4 w-4 accent-(--accent)"
                  />
                  <span className="truncate">{shader.name}</span>
                </label>
              </li>
            );
          })}
        </ul>
      )}
    </Section>
  );
}

export function VideoTab() {
  const v = usePlayer((s) => s.video);
  const hasMedia = usePlayer((s) => s.media !== null) ?? false;
  const renderer = useAppStore((s) => s.info?.capabilities.renderer);
  if (!v) return null;

  return (
    <>
      <Section title="Picture">
        {PICTURE.map(([key, label]) => (
          <ParamRow
            key={key}
            label={label}
            value={v[key]}
            min={-100}
            max={100}
            step={1}
            defaultValue={0}
            format={signed}
            onChange={(value) => set({ [key]: Math.round(value) })}
          />
        ))}
      </Section>

      <Section title="Transform">
        <Segmented
          label="Aspect ratio"
          value={ASPECTS.some((a) => a.value === v.aspect) ? v.aspect : null}
          options={ASPECTS}
          onChange={(aspect) => set({ aspect })}
          fit
        />
        <div className="flex flex-wrap items-center gap-1">
          <Button
            size="sm"
            icon={RotateCcw}
            aria-label="Rotate 90° counter-clockwise"
            onClick={() => set({ rotate: (v.rotate + 270) % 360 })}
          >
            90°
          </Button>
          <Button
            size="sm"
            icon={RotateCw}
            aria-label="Rotate 90° clockwise"
            onClick={() => void dispatch("video.rotate_cw", { trigger: "button" })}
          >
            90°
          </Button>
          <span className="flex-1" />
          <Button
            size="sm"
            icon={FlipHorizontal2}
            aria-pressed={v.flipH}
            className={v.flipH ? "border-accent text-accent-fg" : ""}
            onClick={() => void dispatch("video.flip_horizontal", { trigger: "button" })}
          >
            Mirror
          </Button>
          <Button
            size="sm"
            icon={FlipVertical2}
            aria-pressed={v.flipV}
            className={v.flipV ? "border-accent text-accent-fg" : ""}
            onClick={() => void dispatch("video.flip_vertical", { trigger: "button" })}
          >
            Flip
          </Button>
        </div>
        <ParamRow
          label="Zoom"
          hint="Hold Alt and drag the video to move it around."
          value={v.zoom}
          min={-1}
          max={2}
          step={0.05}
          defaultValue={0}
          scale={["0.5×", "1×", "4×"]}
          format={zoomLabel}
          onChange={(zoom) => set({ zoom })}
        />
      </Section>

      <Section title="HDR" right={<HdrStatus />}>
        <div className="flex items-center gap-2">
          <span className="flex-1 text-base text-fg-2">Tone mapping</span>
          <Select
            label="Tone mapping"
            className="w-40"
            value={v.toneMapping}
            options={toneCurves(renderer)}
            onChange={(toneMapping) => set({ toneMapping })}
          />
        </div>
      </Section>

      <Section title="Enhance">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <Toggle
            label="Debanding"
            hint="Smooths colour banding in gradients."
            checked={v.deband}
            onChange={(deband) => set({ deband })}
          />
          <Toggle
            label="Motion interpolation"
            hint="Smooths motion by blending frames to match your display."
            checked={v.interpolation}
            onChange={(interpolation) => set({ interpolation })}
          />
        </div>
      </Section>

      <Shaders active={v.shaders} />

      <div className="flex justify-end">
        <Button
          variant="ghost"
          disabled={!hasMedia}
          onClick={() => void videoReset().catch(reportError)}
        >
          Reset all video settings
        </Button>
      </div>
    </>
  );
}
