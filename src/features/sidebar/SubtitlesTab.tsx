/**
 * Subtitles tab (DESIGN_SYSTEM §10.1): track and visibility, adding and auto-matching files,
 * sync (delay row plus the visual timeline), and style. Everything acts live; style changes
 * are for this session (defaults live in Settings → Subtitles).
 */
import { Bold, Eye, EyeOff, FilePlus, Link2, X } from "lucide-react";
import { type KeyboardEvent, useEffect, useState } from "react";
import { dispatch } from "@/actions";
import { trackLabel } from "@/actions/definitions/tracks";
import { Button } from "@/components/Button";
import { Chip, Section, Segmented, Select } from "@/components/Controls";
import { IconButton } from "@/components/IconButton";
import { NumberField, ParamRow, useThrottle } from "@/components/ParamRow";
import type { SubMatchReport } from "@/ipc/bindings/SubMatchReport";
import type { SubStyle } from "@/ipc/bindings/SubStyle";
import type { SubStylePatch } from "@/ipc/bindings/SubStylePatch";
import type { Track } from "@/ipc/bindings/Track";
import { subSetDelay, subSetStyle, tracksRemoveExternal, tracksSelect } from "@/ipc/commands";
import { useAutomatchStore } from "@/stores/automatch";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { FontPicker } from "./FontPicker";
import { SubtitleTimeline } from "./SubtitleTimeline";

const NO_TRACKS: Track[] = [];
const OFF = "off";

const SIZES = [
  { value: "0.8", label: "S" },
  { value: "1", label: "M" },
  { value: "1.25", label: "L" },
  { value: "1.5", label: "XL" },
];

const ASS_MODES = [
  { value: "no", label: "Keep", hint: "Show styled subtitles exactly as their author made them." },
  { value: "scale", label: "Scale", hint: "Keep the author's styling, but apply your size." },
  {
    value: "force",
    label: "Override",
    hint: "Use your font, colours, and size for dialogue; signs keep their look and place.",
  },
  { value: "strip", label: "Strip", hint: "Remove all styling and positioning: plain text." },
];

const setStyle = (patch: SubStylePatch) => void subSetStyle(patch).catch(reportError);

function subDetail(track: Track): string {
  return [track.codec.toUpperCase(), track.forced ? "forced" : null, track.external ? "ext" : null]
    .filter(Boolean)
    .join(" ");
}

const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** A colour swatch that opens the system picker, plus the exact hex value. */
function ColorField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (hex: string) => void;
}) {
  const [text, setText] = useState<string | null>(null);
  const send = useThrottle(onChange, 60);
  const commit = () => {
    if (text === null) return;
    const hex = text.trim().startsWith("#") ? text.trim() : `#${text.trim()}`;
    setText(null);
    if (/^#[0-9a-f]{6}$/i.test(hex)) onChange(hex.toUpperCase());
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      commit();
      e.currentTarget.blur();
    } else if (e.key === "Escape") {
      setText(null);
      e.currentTarget.blur();
      e.stopPropagation();
    }
  };
  const valid = /^#[0-9a-f]{6}$/i.test(value);
  return (
    <div className="flex items-center gap-1.5">
      <span className="flex-1 truncate text-base text-fg-2">{label}</span>
      <label
        className="relative h-6 w-6 shrink-0 overflow-hidden rounded-control border border-line-strong"
        style={{ backgroundColor: valid ? value : undefined }}
      >
        <input
          type="color"
          aria-label={`${label}: pick a colour`}
          value={valid ? value.toLowerCase() : "#000000"}
          onChange={(e) => send(e.target.value.toUpperCase())}
          className="absolute inset-0 h-full w-full cursor-pointer opacity-0"
        />
      </label>
      <input
        type="text"
        aria-label={`${label} (hex)`}
        maxLength={7}
        value={text ?? value}
        onFocus={(e) => {
          setText(value);
          requestAnimationFrame(() => e.target.select());
        }}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={onKeyDown}
        className="tabular h-6 w-18 rounded-control border border-line bg-raised px-1.5 font-mono text-fg text-sm uppercase outline-none hover:border-line-strong focus:border-accent"
      />
    </div>
  );
}

/** External files are named by their file; mpv's own title for them is often just "srt". */
function subLabel(track: Track): string {
  return track.external && track.externalPath ? baseName(track.externalPath) : trackLabel(track);
}

function MatchCard({ report }: { report: SubMatchReport }) {
  const running = useAutomatchStore((s) => s.running);
  const run = useAutomatchStore((s) => s.run);
  const dismiss = useAutomatchStore((s) => s.dismiss);
  const total = new Set([
    ...report.matched.map((pair) => pair.video),
    ...report.unmatchedVideos,
    ...report.ambiguous.map((item) => item.video),
  ]).size;
  const problems = report.ambiguous.length + report.unmatchedVideos.length;
  return (
    <div className="flex flex-col gap-2 rounded-control border border-line bg-raised p-2">
      <div className="flex items-center gap-2">
        <span className="flex-1 font-medium text-base text-fg">
          Matched {report.matched.length} of {total} {total === 1 ? "video" : "videos"}
        </span>
        <IconButton icon={X} label="Dismiss" size="panel" onClick={dismiss} />
      </div>
      <details open={problems > 0 && total <= 12}>
        <summary className="cursor-default select-none text-fg-2 text-sm">Details</summary>
        <ul className="mt-1 flex max-h-56 flex-col gap-1 overflow-y-auto text-sm">
          {report.matched.map((pair) => (
            <li key={pair.video} className="flex flex-col">
              <span className="truncate text-fg">{baseName(pair.video)}</span>
              {pair.subs.map((sub) => (
                <span key={sub.path} className="flex items-center gap-1 pl-3 text-fg-2">
                  <span className="min-w-0 flex-1 truncate">→ {baseName(sub.path)}</span>
                  {sub.lang && <Chip>{sub.lang}</Chip>}
                </span>
              ))}
            </li>
          ))}
          {report.ambiguous.map((item) => (
            <li key={`${item.video}-${item.lang ?? ""}`} className="flex flex-col text-warning">
              <span className="truncate">
                {baseName(item.video)}: {item.subs.length} equally likely
                {item.lang ? ` ${item.lang}` : ""} files — not guessed
              </span>
              {item.subs.map((sub) => (
                <span key={sub} className="truncate pl-3">
                  ? {baseName(sub)}
                </span>
              ))}
            </li>
          ))}
          {report.unmatchedVideos.map((video) => (
            <li key={video} className="truncate text-warning">
              {baseName(video)}: no subtitle found
            </li>
          ))}
          {report.unmatchedSubs.length > 0 && (
            <li className="text-fg-3">
              {report.unmatchedSubs.length} subtitle{" "}
              {report.unmatchedSubs.length === 1 ? "file fits" : "files fit"} no video
            </li>
          )}
        </ul>
      </details>
      {report.applied ? (
        <span className="text-fg-2 text-sm">
          Saved. Each video loads its subtitles from now on.
        </span>
      ) : (
        <Button
          variant="primary"
          disabled={running || report.matched.length === 0}
          onClick={() => void run(true)}
          className="self-start"
        >
          Apply
        </Button>
      )}
    </div>
  );
}

function StyleSection({ style, disabled }: { style: SubStyle; disabled: boolean }) {
  const defaults = useConfig((c) => c.subtitles);
  const scale = SIZES.find((s) => Math.abs(Number(s.value) - style.scale) < 0.01)?.value ?? null;
  return (
    <Section title="Style">
      <div className={`flex flex-col gap-2 ${disabled ? "pointer-events-none opacity-50" : ""}`}>
        <div className="flex items-center gap-1.5">
          <span className="w-12 shrink-0 text-base text-fg-2">Font</span>
          <FontPicker value={style.font} onChange={(font) => setStyle({ font })} />
          <IconButton
            icon={Bold}
            label="Bold"
            size="panel"
            on={style.bold}
            onClick={() => setStyle({ bold: !style.bold })}
          />
        </div>
        <div className="flex items-center gap-1.5">
          <span className="w-12 shrink-0 text-base text-fg-2">Size</span>
          <NumberField
            label="Subtitle size"
            value={style.size}
            step={1}
            width="w-12"
            format={(v) => String(Math.round(v))}
            onChange={(size) => setStyle({ size: Math.max(10, Math.min(120, Math.round(size))) })}
          />
          <Segmented
            label="Subtitle size preset"
            className="flex-1"
            options={SIZES}
            value={scale}
            onChange={(v) => setStyle({ scale: Number(v) })}
          />
        </div>
        <ColorField
          label="Text colour"
          value={style.color}
          onChange={(color) => setStyle({ color })}
        />
        <ColorField
          label="Outline colour"
          value={style.outlineColor}
          onChange={(outlineColor) => setStyle({ outlineColor })}
        />
        <ParamRow
          label="Outline size"
          value={style.outlineSize}
          min={0}
          max={10}
          step={0.5}
          defaultValue={defaults?.outline_size ?? 3}
          format={(v) => v.toFixed(1)}
          onChange={(outlineSize) => setStyle({ outlineSize })}
        />
        <div className="flex items-center gap-1.5">
          <span className="flex-1 text-base text-fg-2">Background</span>
          <Segmented
            label="Subtitle background"
            className="w-32"
            options={[
              { value: "none", label: "None" },
              { value: "box", label: "Box" },
            ]}
            value={style.background}
            onChange={(background) => setStyle({ background })}
          />
        </div>
        <ParamRow
          label="Position"
          hint="How far down the picture subtitles sit. 100 % is the bottom."
          value={style.position}
          min={0}
          max={100}
          step={1}
          defaultValue={defaults?.position ?? 100}
          format={(v) => `${Math.round(v)} %`}
          onChange={(position) => setStyle({ position })}
        />
        <div className="flex flex-col gap-1">
          <span className="text-base text-fg-2">Embedded styles (ASS)</span>
          <Segmented
            label="Embedded subtitle styles"
            options={ASS_MODES}
            value={style.assOverride}
            onChange={(assOverride) => setStyle({ assOverride })}
          />
        </div>
      </div>
    </Section>
  );
}

export function SubtitlesTab() {
  const tracks = usePlayer((s) => s.tracks) ?? NO_TRACKS;
  const selected = usePlayer((s) => s.selected.sub) ?? null;
  const visible = usePlayer((s) => s.subVisible) ?? true;
  const delay = usePlayer((s) => s.subDelayMs) ?? 0;
  const style = usePlayer((s) => s.subStyle);
  const hasMedia = usePlayer((s) => s.media !== null) ?? false;
  const isLocal = usePlayer((s) => s.media?.isUrl === false) ?? false;
  const mediaKey = usePlayer((s) => s.media?.key ?? "") ?? "";
  const report = useAutomatchStore((s) => s.report);
  const matching = useAutomatchStore((s) => s.running);
  const dismiss = useAutomatchStore((s) => s.dismiss);
  const subs = tracks.filter((t) => t.kind === "sub");
  const current = subs.find((t) => t.id === selected);

  // A match report describes the file it was made for.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset when the media changes
  useEffect(() => dismiss(), [mediaKey]);

  return (
    <>
      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-1">
          <Select
            label="Subtitle track"
            className="flex-1"
            placeholder={hasMedia ? "Off" : "Nothing playing"}
            value={!hasMedia ? null : selected === null ? OFF : String(selected)}
            options={
              hasMedia
                ? [
                    { value: OFF, label: "Off" },
                    ...subs.map((t) => ({
                      value: String(t.id),
                      label: subLabel(t),
                      hint: subDetail(t),
                    })),
                  ]
                : []
            }
            onChange={(id) =>
              void tracksSelect("sub", id === OFF ? null : Number(id)).catch(reportError)
            }
          />
          {current?.external && (
            <IconButton
              icon={X}
              label="Remove this subtitle file"
              size="panel"
              onClick={() => void tracksRemoveExternal(current.id).catch(reportError)}
            />
          )}
          <IconButton
            icon={visible ? Eye : EyeOff}
            label={visible ? "Hide subtitles" : "Show subtitles"}
            size="panel"
            on={visible && selected !== null}
            actionId="subtitles.toggle_visibility"
          />
        </div>
        <div className="flex flex-wrap gap-1">
          <Button
            size="sm"
            variant="ghost"
            icon={FilePlus}
            disabled={!hasMedia}
            onClick={() => void dispatch("subtitles.add_file", { trigger: "button" })}
          >
            Add file…
          </Button>
          <Button
            size="sm"
            variant="ghost"
            icon={Link2}
            disabled={!isLocal || matching}
            onClick={() => void dispatch("subtitles.automatch", { trigger: "button" })}
          >
            {matching ? "Matching…" : "Auto-match folder"}
          </Button>
        </div>
        {report && <MatchCard report={report} />}
      </div>

      <Section title="Sync">
        <ParamRow
          label="Subtitle delay"
          hint="Positive values show subtitles later."
          value={delay}
          min={-5000}
          max={5000}
          step={10}
          defaultValue={0}
          scale={["−5 s", "0", "+5 s"]}
          format={(ms) => `${ms > 0 ? "+" : ms < 0 ? "−" : ""}${Math.abs(Math.round(ms))} ms`}
          onChange={(ms) => void subSetDelay(Math.round(ms)).catch(reportError)}
          disabled={!hasMedia}
        />
        <SubtitleTimeline />
      </Section>

      {style && <StyleSection style={style} disabled={!hasMedia} />}
    </>
  );
}
