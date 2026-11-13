/**
 * Quick track selectors (DESIGN_SYSTEM §8.3): two bottom-bar buttons that open popovers.
 * Subtitles: Off + tracks with codec/forced/external/default badges, then add/auto-match.
 * Audio: tracks, then output devices; ⚙ opens the Audio tab. Rows are 28 px, ↑/↓ move,
 * Enter selects, and the popover stays open so the change can be seen or heard. A small
 * accent dot marks a non-default state (subtitles on, a chosen output device).
 */
import { AudioLines, Captions, Check, Layers, Settings2 } from "lucide-react";
import { type KeyboardEvent, type ReactNode, useState } from "react";
import { dispatch, isEnabled } from "@/actions";
import { deviceLabel } from "@/actions/definitions/media";
import { trackLabel } from "@/actions/definitions/tracks";
import { Chip } from "@/components/Controls";
import { IconButton } from "@/components/IconButton";
import { Popover } from "@/components/Popover";
import type { Track } from "@/ipc/bindings/Track";
import { audioSetDevice, tracksSelect } from "@/ipc/commands";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { useUiStore } from "@/stores/ui";

const NO_TRACKS: Track[] = [];

function moveFocus(e: KeyboardEvent<HTMLDivElement>) {
  const dir = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
  if (dir === 0) return;
  e.preventDefault();
  e.stopPropagation();
  const rows = [
    ...e.currentTarget.querySelectorAll<HTMLButtonElement>("[data-row]:not(:disabled)"),
  ];
  const index = rows.indexOf(document.activeElement as HTMLButtonElement);
  rows[(index + dir + rows.length) % rows.length]?.focus();
}

function Header({ title, onSettings }: { title: string; onSettings?: () => void }) {
  return (
    <div className="flex h-7 items-center px-2">
      <span className="flex-1 font-medium text-fg-3 text-xs uppercase tracking-[0.06em]">
        {title}
      </span>
      {onSettings && (
        <button
          type="button"
          aria-label={`${title} settings`}
          onClick={onSettings}
          className="flex h-6 w-6 items-center justify-center rounded-control text-fg-2 hover:bg-hover hover:text-fg"
        >
          <Settings2 size={14} strokeWidth={1.5} aria-hidden />
        </button>
      )}
    </div>
  );
}

function Row({
  selected,
  label,
  badges,
  onSelect,
  disabled,
}: {
  selected: boolean;
  label: string;
  badges?: ReactNode;
  onSelect: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      data-row
      role="menuitemradio"
      aria-checked={selected}
      disabled={disabled}
      onClick={onSelect}
      className={`flex h-7 w-full items-center gap-2 rounded-control px-2 text-left text-base outline-none focus-visible:bg-hover disabled:text-fg-off ${
        selected ? "bg-accent-soft text-fg" : "text-fg hover:bg-hover"
      }`}
    >
      <span className="flex w-4 shrink-0 justify-center">
        {selected && <Check size={14} strokeWidth={1.5} className="text-accent-fg" aria-hidden />}
      </span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {badges}
    </button>
  );
}

function ActionRow({ actionId, label }: { actionId: string; label: string }) {
  const enabled = isEnabled(actionId);
  return (
    <button
      type="button"
      data-row
      disabled={!enabled}
      title={enabled ? undefined : "Coming in a later update"}
      onClick={() => void dispatch(actionId, { trigger: "menu" })}
      className="flex h-7 w-full items-center gap-2 rounded-control px-2 text-left text-base text-fg outline-none hover:bg-hover focus-visible:bg-hover disabled:text-fg-off disabled:hover:bg-transparent"
    >
      <span className="w-4" />
      {label}
    </button>
  );
}

function Dot({ on }: { on: boolean }) {
  if (!on) return null;
  return (
    <span
      aria-hidden="true"
      className="pointer-events-none absolute top-1.5 right-1.5 h-1.5 w-1.5 rounded-knob bg-accent"
    />
  );
}

function Selector({
  icon,
  label,
  dot,
  children,
}: {
  icon: typeof Captions;
  label: string;
  dot: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const hold = useUiStore((s) => s.hold);
  return (
    <span className="relative">
      <Popover
        open={open}
        onOpenChange={(next) => {
          setOpen(next);
          hold(`popover-${label}`, next);
        }}
        side="top"
        align="end"
        trigger={<IconButton icon={icon} label={label} keepFocus on={open} onClick={() => {}} />}
      >
        <div role="menu" aria-label={label} onKeyDown={moveFocus}>
          {children}
        </div>
      </Popover>
      <Dot on={dot} />
    </span>
  );
}

function SubtitleRows() {
  const tracks = usePlayer((s) => s.tracks) ?? NO_TRACKS;
  const selected = usePlayer((s) => s.selected.sub) ?? null;
  const subs = tracks.filter((t) => t.kind === "sub");
  const showTab = useUiStore((s) => s.showSidebarTab);
  const select = (id: number | null) => void tracksSelect("sub", id).catch(reportError);
  return (
    <>
      <Header title="Subtitles" onSettings={() => showTab("subtitles")} />
      <Row selected={selected === null} label="Off" onSelect={() => select(null)} />
      {subs.map((t) => (
        <Row
          key={t.id}
          selected={t.id === selected}
          label={trackLabel(t)}
          onSelect={() => select(t.id)}
          badges={
            <span className="flex shrink-0 gap-1">
              <Chip>{t.codec.toUpperCase()}</Chip>
              {t.forced && <Chip>forced</Chip>}
              {t.external && <Chip>ext</Chip>}
              {t.default && <Chip>default</Chip>}
            </span>
          }
        />
      ))}
      <div className="mx-1 my-1 h-px bg-line" />
      <ActionRow actionId="subtitles.add_file" label="Add subtitle file…" />
      <ActionRow actionId="subtitles.automatch" label="Auto-match folder subtitles" />
    </>
  );
}

function AudioRows() {
  const tracks = usePlayer((s) => s.tracks) ?? NO_TRACKS;
  const selected = usePlayer((s) => s.selected.audio) ?? null;
  const device = usePlayer((s) => s.audioDevice) ?? "auto";
  const devices = usePlayer((s) => s.audioDevices) ?? [];
  const showTab = useUiStore((s) => s.showSidebarTab);
  const audio = tracks.filter((t) => t.kind === "audio");
  return (
    <>
      <Header title="Audio track" onSettings={() => showTab("audio")} />
      {audio.length === 0 && (
        <Row selected={false} label="No audio tracks" disabled onSelect={() => {}} />
      )}
      {audio.map((t) => (
        <Row
          key={t.id}
          selected={t.id === selected}
          label={trackLabel(t)}
          onSelect={() => void tracksSelect("audio", t.id).catch(reportError)}
          badges={
            <span className="flex shrink-0 gap-1">
              <Chip>{[t.codec.toUpperCase(), t.channels].filter(Boolean).join(" ")}</Chip>
            </span>
          }
        />
      ))}
      <div className="mx-1 my-1 h-px bg-line" />
      <Header title="Output device" />
      {devices.map((d) => (
        <Row
          key={d.name}
          selected={d.name === device}
          label={deviceLabel(d.name, d.description)}
          onSelect={() => void audioSetDevice(d.name).catch(reportError)}
        />
      ))}
    </>
  );
}

/** Both selectors, or one combined "Tracks" button at narrow widths (§6.4). */
export function TrackSelectors({ combined }: { combined: boolean }) {
  const subOn = usePlayer((s) => s.selected.sub !== null && s.subVisible) ?? false;
  const deviceChosen = usePlayer((s) => s.audioDevice !== "auto") ?? false;
  if (combined) {
    return (
      <Selector icon={Layers} label="Tracks" dot={subOn || deviceChosen}>
        <SubtitleRows />
        <div className="mx-1 my-1 h-px bg-line" />
        <AudioRows />
      </Selector>
    );
  }
  return (
    <>
      <Selector icon={Captions} label="Subtitles" dot={subOn}>
        <SubtitleRows />
      </Selector>
      <Selector icon={AudioLines} label="Audio" dot={deviceChosen}>
        <AudioRows />
      </Selector>
    </>
  );
}
