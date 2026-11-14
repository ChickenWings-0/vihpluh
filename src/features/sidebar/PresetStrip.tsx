/**
 * The preset strip (DESIGN_SYSTEM §10.6), at the bottom of every sidebar tab: the most
 * specific applied preset, a "modified" mark when the live values have moved away from it,
 * Update, Save as…, and a ⋯ menu.
 */
import { Layers, MoreHorizontal } from "lucide-react";
import { type KeyboardEvent, useEffect, useState } from "react";
import { dispatch } from "@/actions";
import { Button } from "@/components/Button";
import { Select } from "@/components/Controls";
import { IconButton } from "@/components/IconButton";
import { DropdownMenu, MenuContent, type MenuEntry, separator } from "@/components/Menu";
import { Popover } from "@/components/Popover";
import type { MatchType } from "@/ipc/bindings/MatchType";
import type { PresetMatch } from "@/ipc/bindings/PresetMatch";
import { presetCaptureCurrent, presetDetach, presetReveal, presetSave } from "@/ipc/commands";
import { reportError, useOsdStore } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { usePresetStore } from "@/stores/presets";
import { useUiStore } from "@/stores/ui";

const SECTIONS = [
  { id: "audio", label: "Audio" },
  { id: "subtitles", label: "Subtitles" },
  { id: "video", label: "Video" },
  { id: "playback", label: "Playback" },
] as const;

const MATCH_LABEL: Record<MatchType, string> = {
  series: "series",
  folder: "folder",
  folder_recursive: "folder tree",
  glob: "pattern",
};

/** The folder of a path, with forward slashes. */
export function folderOf(source: string): string {
  const path = source.replace(/\\/g, "/");
  const cut = path.lastIndexOf("/");
  return cut > 0 ? path.slice(0, cut) : path;
}

const lastSegment = (path: string) => path.split("/").filter(Boolean).pop() ?? path;

/** "sousou no frieren" → "Sousou No Frieren": a starting point for the preset's name. */
export function titleCase(text: string): string {
  return text.replace(/(^|\s)\p{L}/gu, (c) => c.toUpperCase());
}

type Scope = "series" | "folder" | "folder_recursive" | "glob";

function SaveForm({ onDone }: { onDone: () => void }) {
  const source = usePlayer((s) => s.media?.source ?? "") ?? "";
  const isUrl = usePlayer((s) => s.media?.isUrl ?? false) ?? false;
  const series = usePlayer((s) => s.media?.series?.key ?? null) ?? null;
  const folder = folderOf(source);
  // Suggested rule (ARCHITECTURE §10.4): the series when the file is an episode, else its folder.
  const [scope, setScope] = useState<Scope>(isUrl ? "glob" : series ? "series" : "folder");
  const [name, setName] = useState(titleCase(series ?? lastSegment(folder)));
  const [glob, setGlob] = useState(isUrl ? source : `${folder}/**`);
  const [sections, setSections] = useState<string[]>(SECTIONS.map((s) => s.id));
  const [busy, setBusy] = useState(false);

  const scopes: { value: Scope; label: string }[] = [
    ...(series ? [{ value: "series" as const, label: `This series: "${series}"` }] : []),
    ...(isUrl
      ? []
      : [
          { value: "folder" as const, label: "This folder" },
          { value: "folder_recursive" as const, label: "This folder and subfolders" },
        ]),
    { value: "glob", label: "Custom pattern" },
  ];

  const match = (): PresetMatch => {
    if (scope === "series") return { type: "series", series: series ?? "" };
    if (scope === "glob") return { type: "glob", glob: glob.trim() };
    return { type: scope, path: folder };
  };

  const save = async () => {
    if (busy || name.trim() === "" || sections.length === 0) return;
    setBusy(true);
    try {
      const preset = await presetCaptureCurrent(name.trim(), match(), sections);
      const empty = !preset.audio && !preset.subtitles && !preset.video && !preset.playback;
      if (empty) {
        useOsdStore.getState().pushToast({
          kind: "warn",
          text: "Nothing to save: those settings are all at their defaults.",
        });
        return;
      }
      await presetSave(preset);
      useOsdStore.getState().pushToast({ kind: "success", text: `Preset "${preset.name}" saved` });
      onDone();
    } catch (error) {
      reportError(error);
    } finally {
      setBusy(false);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT") {
      e.preventDefault();
      void save();
    }
  };
  const field =
    "h-7 w-full rounded-control border border-line bg-surface px-2 text-base text-fg outline-none placeholder:text-fg-3 focus:border-accent";

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: Enter in a field submits the form
    <div className="flex flex-col gap-2 p-2" onKeyDown={onKeyDown}>
      <label className="flex flex-col gap-1 text-fg-2 text-sm">
        Name
        <input
          type="text"
          // biome-ignore lint/a11y/noAutofocus: the form was opened to be typed into
          autoFocus
          value={name}
          maxLength={80}
          onChange={(e) => setName(e.target.value)}
          className={field}
        />
      </label>
      <div className="flex flex-col gap-1 text-fg-2 text-sm">
        Applies to
        <Select label="Applies to" options={scopes} value={scope} onChange={setScope} />
        {scope === "glob" && (
          <input
            type="text"
            aria-label="Pattern"
            value={glob}
            placeholder="**/Show*/**/*.mkv"
            onChange={(e) => setGlob(e.target.value)}
            className={`${field} font-mono text-sm`}
          />
        )}
      </div>
      <fieldset className="flex flex-col gap-1 text-fg-2 text-sm">
        <legend className="mb-1">Include</legend>
        <div className="grid grid-cols-2 gap-x-2">
          {SECTIONS.map((section) => (
            <label
              key={section.id}
              className="flex h-7 cursor-pointer items-center gap-2 rounded-control px-1 text-base text-fg hover:bg-hover"
            >
              <input
                type="checkbox"
                checked={sections.includes(section.id)}
                onChange={(e) =>
                  setSections(
                    e.target.checked
                      ? [...sections, section.id]
                      : sections.filter((s) => s !== section.id),
                  )
                }
                className="h-4 w-4 accent-(--accent)"
              />
              {section.label}
            </label>
          ))}
        </div>
      </fieldset>
      <Button
        variant="primary"
        keepFocus
        disabled={busy || name.trim() === "" || sections.length === 0}
        onClick={() => void save()}
        className="self-end"
      >
        Save
      </Button>
    </div>
  );
}

export function PresetStrip() {
  const applied = usePlayer((s) => s.appliedPresets) ?? [];
  const dirty = usePlayer((s) => s.presetDirty) ?? false;
  const hasMedia = usePlayer((s) => s.media !== null) ?? false;
  const presets = usePresetStore((s) => s.presets);
  const saveRequest = usePresetStore((s) => s.saveRequest);
  const hold = useUiStore((s) => s.hold);
  const [saving, setSaving] = useState(false);
  const [menu, setMenu] = useState(false);
  const top = applied[applied.length - 1];
  const preset = presets.find((p) => p.id === top);

  const openSave = (open: boolean) => {
    setSaving(open);
    hold("preset-save", open);
  };
  // The `presets.save_current` action asks for the form from anywhere.
  // biome-ignore lint/correctness/useExhaustiveDependencies: only a new request opens it
  useEffect(() => {
    if (saveRequest > 0 && hasMedia) openSave(true);
  }, [saveRequest]);

  const entries: MenuEntry[] = [
    {
      kind: "item",
      label: "Detach for this file",
      disabled: applied.length === 0,
      onSelect: () => void presetDetach().catch(reportError),
    },
    separator,
    {
      kind: "item",
      label: "Manage presets…",
      onSelect: () => useUiStore.getState().openSettings("presets"),
    },
    {
      kind: "item",
      label: "Reveal preset file",
      disabled: !top,
      onSelect: () => top && void presetReveal(top).catch(reportError),
    },
  ];

  return (
    <div
      data-preset-strip
      className="flex h-10 shrink-0 items-center gap-1 border-line border-t bg-raised pr-1 pl-3"
    >
      <Layers
        size={14}
        strokeWidth={1.5}
        className={`shrink-0 ${top ? "text-accent-fg" : "text-fg-3"}`}
        aria-hidden
      />
      <span className="flex min-w-0 flex-1 items-baseline gap-1.5">
        <span className={`truncate text-base ${top ? "text-fg" : "text-fg-3"}`}>
          {top ? (preset?.name ?? top) : "No preset"}
        </span>
        {preset && (
          <span className="shrink-0 text-fg-3 text-xs">{MATCH_LABEL[preset.matchType]}</span>
        )}
        {applied.length > 1 && (
          <span className="shrink-0 text-fg-3 text-xs" title={applied.join(" → ")}>
            +{applied.length - 1}
          </span>
        )}
        {dirty && (
          <span className="flex shrink-0 items-center gap-1 text-warning text-xs">
            <span className="h-1.5 w-1.5 rounded-knob bg-warning" aria-hidden />
            modified
          </span>
        )}
      </span>
      {top && dirty && (
        <Button
          size="sm"
          onClick={() => void dispatch("presets.update_current", { trigger: "button" })}
        >
          Update
        </Button>
      )}
      <Popover
        open={saving}
        onOpenChange={openSave}
        side="top"
        align="end"
        width="w-72"
        trigger={
          <Button size="sm" variant="ghost" keepFocus disabled={!hasMedia}>
            Save as…
          </Button>
        }
      >
        <SaveForm onDone={() => openSave(false)} />
      </Popover>
      <DropdownMenu.Root
        open={menu}
        modal={false}
        onOpenChange={(open) => {
          setMenu(open);
          hold("preset-menu", open);
        }}
      >
        <DropdownMenu.Trigger asChild>
          <IconButton
            icon={MoreHorizontal}
            label="Preset options"
            size="panel"
            keepFocus
            on={menu}
            onClick={() => {}}
          />
        </DropdownMenu.Trigger>
        <MenuContent entries={entries} side="top" align="end" />
      </DropdownMenu.Root>
    </div>
  );
}
