/**
 * Presets manager (KEYBINDS_AND_CONFIG §5.4), the one sub-page of Settings → Playback: every
 * preset with its on/off switch, what it applies to, and delete / reveal. Editing a rule
 * rewrites only the `[match]` table of the preset's file; the values it sets are changed
 * from the preset strip of the side panel, where they can be heard and seen.
 */
import { FolderOpen, Trash2 } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/Button";
import { Chip, Select, Toggle } from "@/components/Controls";
import { Tooltip } from "@/components/Tooltip";
import type { MatchType } from "@/ipc/bindings/MatchType";
import type { PresetMatch } from "@/ipc/bindings/PresetMatch";
import type { PresetSummary } from "@/ipc/bindings/PresetSummary";
import { presetDelete, presetGet, presetReveal, presetSave } from "@/ipc/commands";
import { reportError } from "@/stores/osd";
import { usePresetStore } from "@/stores/presets";

const MATCH_TYPES: { value: MatchType; label: string }[] = [
  { value: "series", label: "Series" },
  { value: "folder", label: "Folder" },
  { value: "folder_recursive", label: "Folder and subfolders" },
  { value: "glob", label: "Pattern" },
];

const PLACEHOLDER: Record<MatchType, string> = {
  series: "series name, as in the file names",
  folder: "a folder path",
  folder_recursive: "a folder path",
  glob: "**/*.mkv",
};

export function matchOf(type: MatchType, value: string): PresetMatch {
  if (type === "series") return { type, series: value };
  if (type === "glob") return { type, glob: value };
  return { type, path: value };
}

/** Changes one preset through read-modify-write, so the file keeps everything else. */
async function update(id: string, change: (preset: Awaited<ReturnType<typeof presetGet>>) => void) {
  const preset = await presetGet(id);
  change(preset);
  await presetSave(preset);
}

function PresetRow({ preset }: { preset: PresetSummary }) {
  const [value, setValue] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const commit = () => {
    if (value === null) return;
    const next = value.trim();
    setValue(null);
    if (next === "" || next === preset.matchValue) return;
    void update(preset.id, (p) => {
      p.match = matchOf(preset.matchType, next);
    }).catch(reportError);
  };
  return (
    <li data-preset={preset.id} className="flex flex-col gap-1.5 border-line border-b py-2.5">
      <div className="flex items-center gap-2">
        <Toggle
          checked={preset.enabled}
          label={`${preset.name}: apply automatically`}
          showLabel={false}
          onChange={(enabled) =>
            void update(preset.id, (p) => {
              p.enabled = enabled;
            }).catch(reportError)
          }
        />
        <span
          className={`min-w-0 flex-1 truncate font-medium text-base ${preset.enabled ? "text-fg" : "text-fg-3"}`}
        >
          {preset.name}
        </span>
        {preset.sections.map((section) => (
          <Chip key={section}>{section}</Chip>
        ))}
        <Tooltip label="Show the preset's file">
          <button
            type="button"
            aria-label={`Show the file of ${preset.name}`}
            onClick={() => void presetReveal(preset.id).catch(reportError)}
            className="flex h-7 w-7 items-center justify-center rounded-control text-fg-2 hover:bg-hover hover:text-fg"
          >
            <FolderOpen size={16} strokeWidth={1.5} aria-hidden />
          </button>
        </Tooltip>
        <Tooltip label="Delete preset">
          <button
            type="button"
            aria-label={`Delete ${preset.name}`}
            onClick={() => setConfirming(true)}
            className="flex h-7 w-7 items-center justify-center rounded-control text-fg-2 hover:bg-danger-soft hover:text-danger"
          >
            <Trash2 size={16} strokeWidth={1.5} aria-hidden />
          </button>
        </Tooltip>
      </div>
      <div className="flex items-center gap-2 pl-10">
        <span className="shrink-0 text-fg-2 text-sm">Applies to</span>
        <Select
          options={MATCH_TYPES}
          value={preset.matchType}
          label={`${preset.name}: what it applies to`}
          className="w-44"
          onChange={(type) =>
            void update(preset.id, (p) => {
              p.match = matchOf(type, preset.matchValue);
            }).catch(reportError)
          }
        />
        <input
          type="text"
          spellCheck={false}
          aria-label={`${preset.name}: ${PLACEHOLDER[preset.matchType]}`}
          value={value ?? preset.matchValue}
          placeholder={PLACEHOLDER[preset.matchType]}
          onChange={(e) => setValue(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
            else if (e.key === "Escape" && value !== null) {
              e.stopPropagation();
              setValue(null);
            }
          }}
          className="h-7 min-w-0 flex-1 rounded-control border border-line bg-raised px-2 font-mono text-fg text-sm outline-none placeholder:text-fg-3 hover:border-line-strong focus:border-accent"
        />
      </div>
      {confirming && (
        <div
          role="alert"
          className="ml-10 flex items-center gap-2 rounded-control border border-line-strong bg-raised px-2 py-1.5 text-base text-fg"
        >
          <span className="min-w-0 flex-1">Delete “{preset.name}” and its file?</span>
          <Button
            variant="danger"
            keepFocus
            onClick={() => {
              setConfirming(false);
              void presetDelete(preset.id).catch(reportError);
            }}
          >
            Delete
          </Button>
          <Button variant="ghost" keepFocus onClick={() => setConfirming(false)}>
            Cancel
          </Button>
        </div>
      )}
    </li>
  );
}

export function PresetsManager() {
  const presets = usePresetStore((s) => s.presets);
  if (presets.length === 0) {
    return (
      <p className="py-8 text-center text-base text-fg-3">
        No presets yet. Set up a file the way you like it, then use “Save as…” on the bar at the
        bottom of the side panel.
      </p>
    );
  }
  return (
    <ul data-presets className="flex flex-col">
      {presets.map((preset) => (
        <PresetRow key={preset.id} preset={preset} />
      ))}
    </ul>
  );
}
