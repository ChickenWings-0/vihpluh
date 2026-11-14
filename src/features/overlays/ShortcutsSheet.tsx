/**
 * Shortcuts sheet (KEYBINDS_AND_CONFIG §4, `F1`): a read-only, searchable, two-column list of
 * every bound action in the current keymap, built from the same rows as the hotkey editor.
 */
import { Search, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { allActions } from "@/actions";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { PARAMETERISED } from "@/actions/parameterised";
import { Button } from "@/components/Button";
import { Chip } from "@/components/Controls";
import { KeyCap } from "@/components/KeyCap";
import { useKeymapStore } from "@/stores/keymap";
import { useUiStore } from "@/stores/ui";
import { buildGroups, customLabel, filterGroups } from "../settings/hotkeys";

export function ShortcutsSheet() {
  const open = useUiStore((s) => s.overlay === "shortcuts");
  const setOverlay = useUiStore((s) => s.setOverlay);
  const openSettings = useUiStore((s) => s.openSettings);
  const hold = useUiStore((s) => s.hold);
  const keymap = useKeymapStore((s) => s.keymap);
  const [query, setQuery] = useState("");
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    hold("shortcuts", true);
    setQuery("");
    input.current?.focus();
    return () => hold("shortcuts", false);
  }, [open, hold]);

  const groups = useMemo(() => {
    if (!keymap) return [];
    const actions = allActions().map((a) => ({ id: a.id, label: a.label, group: a.group }));
    const bound = buildGroups(keymap, actions, PARAMETERISED)
      .map((group) => ({ ...group, rows: group.rows.filter((row) => row.keys.length > 0) }))
      .filter((group) => group.rows.length > 0);
    return filterGroups(bound, query);
  }, [keymap, query]);
  const custom = useMemo(() => {
    const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
    return (keymap?.custom ?? []).filter((binding) =>
      tokens.every((token) => customLabel(binding).toLowerCase().includes(token)),
    );
  }, [keymap, query]);

  if (!open) return null;
  const close = () => {
    setOverlay(null);
    focusPlayerRoot();
  };

  return (
    <div
      className="absolute inset-0 z-(--z-overlay) flex items-center justify-center bg-scrim"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Keyboard shortcuts"
        data-overlay="shortcuts"
        className="flex h-[calc(100%-32px)] max-h-160 w-[calc(100%-32px)] max-w-230 flex-col overflow-hidden rounded-surface border border-line-strong bg-surface shadow-float"
      >
        <header className="flex h-12 shrink-0 items-center gap-3 border-line border-b pr-2 pl-5">
          <h2 className="font-medium text-fg text-lg">Keyboard shortcuts</h2>
          <label className="ml-auto flex h-7 w-64 items-center gap-2 rounded-control border border-line bg-raised px-2 focus-within:border-accent">
            <Search size={14} strokeWidth={1.5} className="shrink-0 text-fg-3" aria-hidden />
            <input
              ref={input}
              type="text"
              value={query}
              placeholder="Search…"
              aria-label="Search shortcuts"
              spellCheck={false}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape" && query !== "") {
                  e.preventDefault();
                  e.stopPropagation();
                  setQuery("");
                }
              }}
              className="h-full min-w-0 flex-1 bg-transparent text-base text-fg outline-none placeholder:text-fg-3"
            />
          </label>
          <Button variant="ghost" keepFocus onClick={() => openSettings("controls")}>
            Edit…
          </Button>
          <button
            type="button"
            aria-label="Close keyboard shortcuts"
            onClick={close}
            className="flex h-7 w-7 items-center justify-center rounded-control text-fg-2 hover:bg-hover hover:text-fg"
          >
            <X size={16} strokeWidth={1.5} aria-hidden />
          </button>
        </header>
        <div className="min-h-0 flex-1 columns-2 gap-8 overflow-y-auto px-5 py-3">
          {groups.map((group) => (
            <section
              key={group.id}
              aria-label={group.label}
              className="mb-3 break-inside-avoid-column"
            >
              <h3 className="flex h-7 items-center gap-2 font-medium text-fg-3 text-xs uppercase tracking-[0.06em]">
                {group.label}
                {group.context && <Chip>panel</Chip>}
              </h3>
              {group.rows.map((row) => (
                <div
                  key={`${row.context}:${row.actionId}`}
                  data-shortcut={row.actionId}
                  className="flex min-h-7 items-center gap-2 py-0.5"
                >
                  <span className="min-w-0 flex-1 truncate text-base text-fg">{row.label}</span>
                  <span className="flex flex-wrap justify-end gap-1">
                    {row.keys.map((key) => (
                      <KeyCap key={key} binding={key} />
                    ))}
                  </span>
                </div>
              ))}
            </section>
          ))}
          {custom.length > 0 && (
            <section aria-label="Custom" className="mb-3 break-inside-avoid-column">
              <h3 className="flex h-7 items-center font-medium text-fg-3 text-xs uppercase tracking-[0.06em]">
                Custom
              </h3>
              {custom.map((binding) => (
                <div key={binding.index} className="flex min-h-7 items-center gap-2 py-0.5">
                  <span className="min-w-0 flex-1 truncate text-base text-fg">
                    {customLabel(binding)}
                  </span>
                  <span className="flex flex-wrap justify-end gap-1">
                    {binding.keys.map((key) => (
                      <KeyCap key={key} binding={key} />
                    ))}
                  </span>
                </div>
              ))}
            </section>
          )}
          {groups.length === 0 && custom.length === 0 && (
            <p className="py-8 text-center text-base text-fg-3 [column-span:all]">
              No shortcut matches.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
