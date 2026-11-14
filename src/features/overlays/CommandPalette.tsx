/**
 * Command palette (`Ctrl+Shift+P`): every action that can run right now, fuzzy-searchable,
 * with its shortcut. Choosing one dispatches it exactly as its button or key would.
 */
import { Search } from "lucide-react";
import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { allActions, dispatch, isEnabled } from "@/actions";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { PARAMETERISED } from "@/actions/parameterised";
import type { Action } from "@/actions/types";
import { KeyCap } from "@/components/KeyCap";
import { fuzzyFilter } from "@/lib/fuzzy";
import { useKeymapStore } from "@/stores/keymap";
import { useUiStore } from "@/stores/ui";

const GROUP_LABELS: Record<string, string> = {
  playback: "Playback",
  queue: "Queue",
  audio: "Audio",
  subtitles: "Subtitles",
  video: "Video",
  window: "Window",
  app: "App",
};

/** What the palette offers: enabled, argument-free actions that are not panel-scoped. */
export function paletteActions(actions: readonly Action[]): Action[] {
  return actions.filter(
    (a) =>
      a.group !== "context" &&
      a.id !== "ui.command_palette" &&
      a.id !== "ui.escape" &&
      a.id !== "ui.context_menu" &&
      !PARAMETERISED.includes(a.id) &&
      isEnabled(a.id),
  );
}

export function CommandPalette() {
  const open = useUiStore((s) => s.overlay === "palette");
  const setOverlay = useUiStore((s) => s.setOverlay);
  const hold = useUiStore((s) => s.hold);
  const keysOf = useKeymapStore((s) => s.lookup.keysOf);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLUListElement>(null);

  useEffect(() => {
    if (!open) return;
    hold("palette", true);
    setQuery("");
    setActive(0);
    input.current?.focus();
    return () => hold("palette", false);
  }, [open, hold]);

  // The list is made when the palette opens: what is enabled depends on the moment.
  const candidates = useMemo(() => (open ? paletteActions(allActions()) : []), [open]);
  const shown = useMemo(
    () => fuzzyFilter(query, candidates, (a) => `${a.label} ${GROUP_LABELS[a.group] ?? ""}`),
    [query, candidates],
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: follows the highlighted row
  useEffect(() => {
    list.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
  }, [active, shown]);

  if (!open) return null;
  const close = () => {
    setOverlay(null);
    focusPlayerRoot();
  };
  const run = (action: Action | undefined) => {
    if (!action) return;
    close();
    void dispatch(action.id, { trigger: "palette" });
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    const last = shown.length - 1;
    if (e.key === "ArrowDown") setActive(active >= last ? 0 : active + 1);
    else if (e.key === "ArrowUp") setActive(active <= 0 ? last : active - 1);
    else if (e.key === "PageDown") setActive(Math.min(last, active + 8));
    else if (e.key === "PageUp") setActive(Math.max(0, active - 8));
    else if (e.key === "Enter") run(shown[active]);
    else return;
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <div
      className="absolute inset-0 z-(--z-overlay) flex items-start justify-center bg-scrim pt-[12vh]"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        data-overlay="palette"
        className="flex max-h-[70vh] w-140 max-w-[calc(100%-32px)] flex-col overflow-hidden rounded-surface border border-line-strong bg-raised shadow-float"
      >
        <label className="flex h-10 shrink-0 items-center gap-2 border-line border-b px-3">
          <Search size={16} strokeWidth={1.5} className="shrink-0 text-fg-3" aria-hidden />
          <input
            ref={input}
            type="text"
            role="combobox"
            aria-expanded="true"
            aria-controls="palette-list"
            aria-activedescendant={shown[active] ? `palette-${shown[active].id}` : undefined}
            aria-label="Run a command"
            value={query}
            placeholder="Type a command…"
            spellCheck={false}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
            onKeyDown={onKeyDown}
            className="h-full min-w-0 flex-1 bg-transparent text-fg text-md outline-none placeholder:text-fg-3"
          />
        </label>
        <ul
          ref={list}
          id="palette-list"
          // biome-ignore lint/a11y/noNoninteractiveElementToInteractiveRole: the list a combobox controls
          role="listbox"
          aria-label="Commands"
          className="min-h-0 flex-1 overflow-y-auto p-1"
        >
          {shown.map((action, index) => {
            const Icon = action.icon;
            const keys = keysOf.get(action.id) ?? [];
            return (
              // biome-ignore lint/a11y/useKeyWithClickEvents: keys are handled by the combobox input
              <li
                key={action.id}
                id={`palette-${action.id}`}
                // biome-ignore lint/a11y/noNoninteractiveElementToInteractiveRole: an option of the listbox
                role="option"
                tabIndex={-1}
                aria-selected={index === active}
                data-command={action.id}
                onPointerMove={() => {
                  if (index !== active) setActive(index);
                }}
                onClick={() => run(action)}
                className={`flex h-8 items-center gap-2 rounded-control px-2 text-base text-fg ${
                  index === active ? "bg-hover" : ""
                }`}
              >
                <span className="flex w-4 shrink-0 justify-center text-fg-2">
                  {Icon && <Icon size={16} strokeWidth={1.5} aria-hidden />}
                </span>
                <span className="min-w-0 flex-1 truncate">{action.label}</span>
                <span className="text-fg-3 text-xs">{GROUP_LABELS[action.group]}</span>
                {keys[0] && <KeyCap binding={keys[0]} />}
              </li>
            );
          })}
          {shown.length === 0 && (
            <li className="py-6 text-center text-base text-fg-3">No command matches.</li>
          )}
        </ul>
      </div>
    </div>
  );
}
