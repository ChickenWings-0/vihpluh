/**
 * Audio tab (DESIGN_SYSTEM §10.2): track, output device, preferred languages, audio delay,
 * the equalizer, and volume normalisation. Every control acts live.
 */
import { X } from "lucide-react";
import { type KeyboardEvent, useState } from "react";
import { dispatch } from "@/actions";
import { deviceLabel } from "@/actions/definitions/media";
import { trackLabel } from "@/actions/definitions/tracks";
import { Section, Select, Toggle } from "@/components/Controls";
import { ParamRow } from "@/components/ParamRow";
import type { Track } from "@/ipc/bindings/Track";
import { audioSetDelay, audioSetDevice, configSet, tracksSelect } from "@/ipc/commands";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { Equalizer } from "./Equalizer";

const NO_TRACKS: Track[] = [];

export function trackDetail(track: Track): string {
  return [track.codec.toUpperCase(), track.channels, track.external ? "ext" : null]
    .filter(Boolean)
    .join(" ");
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <span className="w-18 shrink-0 font-medium text-fg-3 text-xs uppercase tracking-[0.06em]">
        {label}
      </span>
      <div className="flex min-w-0 flex-1">{children}</div>
    </div>
  );
}

/** `audio.preferred_languages` as removable chips plus an add field. */
function LanguageChips() {
  const languages = useConfig((c) => c.audio.preferred_languages) ?? [];
  const [draft, setDraft] = useState("");
  const save = (next: string[]) =>
    void configSet("audio.preferred_languages", next).catch(reportError);
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "Enter" && e.key !== ",") return;
    e.preventDefault();
    const code = draft.trim().toLowerCase();
    if (/^[a-z]{2,3}$/.test(code) && !languages.includes(code)) save([...languages, code]);
    setDraft("");
  };
  return (
    <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1">
      {languages.map((code) => (
        <span
          key={code}
          className="inline-flex h-6 items-center gap-0.5 rounded-control bg-active pr-0.5 pl-1.5 font-mono text-fg text-sm"
        >
          {code}
          <button
            type="button"
            aria-label={`Remove ${code}`}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => save(languages.filter((l) => l !== code))}
            className="flex h-5 w-5 items-center justify-center rounded-control text-fg-2 hover:bg-hover hover:text-fg"
          >
            <X size={12} strokeWidth={1.5} aria-hidden />
          </button>
        </span>
      ))}
      <input
        type="text"
        value={draft}
        maxLength={3}
        placeholder="add"
        aria-label="Add a preferred audio language (code like eng, jpn)"
        title="Preferred audio languages: the first available one is chosen. Use codes like eng, jpn."
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={onKeyDown}
        className="h-6 w-12 rounded-control border border-line bg-raised px-1.5 font-mono text-fg text-sm outline-none placeholder:text-fg-3 focus:border-accent"
      />
    </div>
  );
}

export function AudioTab() {
  const tracks = usePlayer((s) => s.tracks) ?? NO_TRACKS;
  const selected = usePlayer((s) => s.selected.audio) ?? null;
  const device = usePlayer((s) => s.audioDevice) ?? "auto";
  const devices = usePlayer((s) => s.audioDevices) ?? [];
  const delay = usePlayer((s) => s.audioDelayMs) ?? 0;
  const normalize = usePlayer((s) => s.normalize) ?? false;
  const hasMedia = usePlayer((s) => s.media !== null) ?? false;
  const audio = tracks.filter((t) => t.kind === "audio");

  return (
    <>
      <div className="flex flex-col gap-2">
        <Field label="Track">
          <Select
            label="Audio track"
            className="flex-1"
            placeholder={hasMedia ? "No audio" : "Nothing playing"}
            value={selected === null ? null : String(selected)}
            options={audio.map((t) => ({
              value: String(t.id),
              label: trackLabel(t),
              hint: trackDetail(t),
            }))}
            onChange={(id) => void tracksSelect("audio", Number(id)).catch(reportError)}
          />
        </Field>
        <Field label="Output">
          <Select
            label="Output device"
            className="flex-1"
            value={device}
            options={devices.map((d) => ({
              value: d.name,
              label: deviceLabel(d.name, d.description),
            }))}
            onChange={(name) => void audioSetDevice(name).catch(reportError)}
          />
        </Field>
        <Field label="Languages">
          <LanguageChips />
        </Field>
      </div>

      <Section title="Sync">
        <ParamRow
          label="Audio delay"
          value={delay}
          min={-5000}
          max={5000}
          step={10}
          defaultValue={0}
          scale={["−5 s", "0", "+5 s"]}
          format={(ms) => `${ms > 0 ? "+" : ms < 0 ? "−" : ""}${Math.abs(Math.round(ms))} ms`}
          onChange={(ms) => void audioSetDelay(Math.round(ms)).catch(reportError)}
          disabled={!hasMedia}
        />
      </Section>

      <Equalizer />

      <div className="flex items-center justify-between">
        <span className="text-base text-fg-2">Volume normalisation</span>
        <Toggle
          label="Volume normalisation"
          showLabel={false}
          hint="Evens out loud and quiet moments."
          checked={normalize}
          onChange={() => void dispatch("audio.toggle_normalize", { trigger: "button" })}
        />
      </div>
    </>
  );
}
