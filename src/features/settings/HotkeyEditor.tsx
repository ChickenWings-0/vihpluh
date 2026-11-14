/**
 * The hotkey editor (KEYBINDS_AND_CONFIG §4), hosted by Settings → Controls. Rows grouped as
 * the action reference; `+` or a click on a key cap captures the next key, mouse button, or
 * wheel notch; a key already in use shows the conflict bar and changes nothing until chosen.
 * Every change goes to `keybinds.toml` at once through `keybinds_set`, and the keymap store
 * (fed by `keybinds://changed`) is the only thing rendered from.
 */
import { Keyboard, Plus, RotateCcw, Search, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { allActions, getAction } from "@/actions";
import { PARAMETERISED } from "@/actions/parameterised";
import { Button } from "@/components/Button";
import { Chip, Select } from "@/components/Controls";
import { KeyCap } from "@/components/KeyCap";
import { Tooltip } from "@/components/Tooltip";
import { keybindsRemoveCustom, keybindsReset, keybindsSet, keybindsSetBase } from "@/ipc/commands";
import {
  bindingFromKeyboard,
  bindingFromMouse,
  bindingFromWheel,
  displayBinding,
} from "@/keybinds/normalize";
import { useKeymapStore } from "@/stores/keymap";
import { reportError } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";
import {
  buildGroups,
  CONTEXT_LABELS,
  type Conflict,
  customForKey,
  customLabel,
  filterGroups,
  findConflict,
  groupsForKey,
  type HotkeyRow,
  RESERVED,
  resolveConflict,
  withKey,
  withoutKey,
} from "./hotkeys";

const BASES = [
  { value: "vihpluh", label: "vihpluh" },
  { value: "vlc", label: "VLC-style" },
  { value: "mpv", label: "mpv-style" },
  { value: "none", label: "Empty" },
];

/** What is waiting for a key: a row (adding, or replacing one cap) or the find-by-key box. */
type Capture =
  | { kind: "row"; context: string; actionId: string; replacing?: string }
  | { kind: "find" };

interface Pending {
  context: string;
  actionId: string;
  replacing?: string;
  conflict: Conflict;
}

const rowId = (context: string, actionId: string) => `${context}:${actionId}`;
const write = (context: string, actionId: string, keys: string[]) =>
  keybindsSet(context, actionId, keys).catch(reportError);

/**
 * Listens for the next binding while `active`. Keys are taken wherever focus is; mouse
 * buttons and the wheel only over the element that carries `data-capture-target`, so the
 * rest of the editor stays clickable (a click elsewhere cancels).
 */
function useCapture(
  active: boolean,
  onBinding: (binding: string) => void,
  onCancel: () => void,
): void {
  const handlers = useRef({ onBinding, onCancel });
  handlers.current = { onBinding, onCancel };
  useEffect(() => {
    if (!active) return;
    const overTarget = (e: Event) =>
      e.target instanceof Element && e.target.closest("[data-capture-target]") !== null;
    const stop = (e: Event) => {
      e.preventDefault();
      e.stopPropagation();
    };
    let clickTimer: ReturnType<typeof setTimeout> | undefined;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.isComposing || e.repeat) return;
      const binding = bindingFromKeyboard(e);
      if (!binding) return; // a modifier on its own: keep waiting
      stop(e);
      if (binding === "Escape") handlers.current.onCancel();
      else handlers.current.onBinding(binding);
    };
    const onMouseDown = (e: MouseEvent) => {
      if (!overTarget(e)) {
        handlers.current.onCancel();
        return;
      }
      stop(e);
      if (e.button !== 0) {
        const binding = bindingFromMouse(e);
        if (binding) handlers.current.onBinding(binding);
        return;
      }
      // A second press within the double-click time makes it the double-click binding.
      if (clickTimer !== undefined) {
        clearTimeout(clickTimer);
        clickTimer = undefined;
        const binding = bindingFromMouse(e, true);
        if (binding) handlers.current.onBinding(binding);
        return;
      }
      const single = bindingFromMouse(e);
      clickTimer = setTimeout(() => {
        clickTimer = undefined;
        if (single) handlers.current.onBinding(single);
      }, 300);
    };
    const onWheel = (e: WheelEvent) => {
      if (!overTarget(e)) return;
      stop(e);
      const binding = bindingFromWheel(e);
      if (binding) handlers.current.onBinding(binding);
    };
    const onContextMenu = (e: MouseEvent) => {
      if (overTarget(e)) stop(e);
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("mousedown", onMouseDown, true);
    window.addEventListener("wheel", onWheel, { capture: true, passive: false });
    window.addEventListener("contextmenu", onContextMenu, true);
    return () => {
      clearTimeout(clickTimer);
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("mousedown", onMouseDown, true);
      window.removeEventListener("wheel", onWheel, true);
      window.removeEventListener("contextmenu", onContextMenu, true);
    };
  }, [active]);
}

export function HotkeyEditor() {
  const keymap = useKeymapStore((s) => s.keymap);
  const setCapturingKey = useUiStore((s) => s.setCapturingKey);
  const [query, setQuery] = useState("");
  const [capture, setCapture] = useState<Capture | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [refusal, setRefusal] = useState<{ row: string; text: string } | null>(null);
  const [foundKey, setFoundKey] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);

  const actions = useMemo(
    () => allActions().map((a) => ({ id: a.id, label: a.label, group: a.group })),
    [],
  );
  const groups = useMemo(
    () => (keymap ? buildGroups(keymap, actions, PARAMETERISED) : []),
    [keymap, actions],
  );
  const shown = useMemo(
    () => (foundKey ? groupsForKey(groups, foundKey) : filterGroups(groups, query)),
    [groups, foundKey, query],
  );

  // Shortcuts must not fire while a key is being recorded.
  useEffect(() => {
    setCapturingKey(capture !== null);
    return () => setCapturingKey(false);
  }, [capture, setCapturingKey]);

  useCapture(
    capture !== null,
    (binding) => {
      const target = capture;
      setCapture(null);
      if (!target || !keymap) return;
      if (target.kind === "find") {
        setFoundKey(binding);
        return;
      }
      const { context, actionId, replacing } = target;
      const reason = RESERVED[binding];
      if (reason) {
        setRefusal({ row: rowId(context, actionId), text: reason });
        return;
      }
      const conflict = findConflict(keymap, context, actionId, binding);
      if (conflict) {
        setPending({ context, actionId, replacing, conflict });
        return;
      }
      const keys = keymap.contexts[context]?.[actionId] ?? [];
      void write(context, actionId, withKey(keys, binding, replacing));
    },
    () => setCapture(null),
  );

  if (!keymap) return null;

  const startCapture = (row: HotkeyRow, replacing?: string) => {
    setPending(null);
    setRefusal(null);
    setCapture({ kind: "row", context: row.context, actionId: row.actionId, replacing });
  };
  const resolve = (choice: "replace" | "neither") => {
    if (!pending) return;
    const writes = resolveConflict(keymap, pending, pending.conflict, choice);
    setPending(null);
    void (async () => {
      for (const w of writes) await write(w.context, w.actionId, w.keys);
    })();
  };
  const custom = foundKey
    ? customForKey(keymap.custom, foundKey)
    : keymap.custom.filter((binding) =>
        query
          .toLowerCase()
          .split(/\s+/)
          .filter(Boolean)
          .every((token) => customLabel(binding).toLowerCase().includes(token)),
      );
  const anyModified = groups.some((g) => g.rows.some((r) => r.modified));
  const finding = capture?.kind === "find";

  return (
    <section aria-label="Shortcuts" data-hotkeys className="flex flex-col gap-2">
      <div className="sticky top-0 z-10 flex flex-col gap-2 bg-surface pt-1 pb-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-base text-fg-2">Keymap</span>
          <Select
            options={BASES}
            value={keymap.base}
            onChange={(base) => void keybindsSetBase(base).catch(reportError)}
            label="Keymap"
            className="w-32"
          />
          <label className="flex h-7 min-w-40 flex-1 items-center gap-2 rounded-control border border-line bg-raised px-2 focus-within:border-accent">
            <Search size={14} strokeWidth={1.5} className="shrink-0 text-fg-3" aria-hidden />
            <input
              type="text"
              value={query}
              placeholder="Search actions…"
              aria-label="Search actions"
              onChange={(e) => {
                setQuery(e.target.value);
                setFoundKey(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Escape" && query !== "") {
                  e.stopPropagation();
                  setQuery("");
                }
              }}
              className="h-full min-w-0 flex-1 bg-transparent text-base text-fg outline-none placeholder:text-fg-3"
            />
          </label>
          <Button
            icon={Keyboard}
            keepFocus
            aria-pressed={finding || foundKey !== null}
            data-capture-target={finding ? "" : undefined}
            className={finding || foundKey ? "border-accent text-accent-fg" : ""}
            onClick={() => {
              setPending(null);
              setRefusal(null);
              if (foundKey) setFoundKey(null);
              else setCapture(finding ? null : { kind: "find" });
            }}
          >
            {finding ? "Press a key…" : "Find by key"}
          </Button>
          <Button
            variant="ghost"
            keepFocus
            disabled={!anyModified}
            onClick={() => setConfirmReset(true)}
          >
            Reset all…
          </Button>
        </div>
        {foundKey && (
          <div className="flex items-center gap-2 text-fg-2 text-sm">
            <span>Showing what</span>
            <KeyCap binding={foundKey} />
            <span>triggers.</span>
            <Button size="sm" variant="ghost" keepFocus onClick={() => setFoundKey(null)}>
              Show all
            </Button>
          </div>
        )}
        {confirmReset && (
          <div
            role="alert"
            className="flex flex-wrap items-center gap-2 rounded-control border border-line-strong bg-raised px-2 py-1.5 text-base text-fg"
          >
            <span className="min-w-0 flex-1">
              Put every shortcut back to the {BASES.find((b) => b.value === keymap.base)?.label}{" "}
              keymap? Custom bindings are kept.
            </span>
            <Button
              variant="danger"
              keepFocus
              onClick={() => {
                setConfirmReset(false);
                void keybindsReset().catch(reportError);
              }}
            >
              Reset all
            </Button>
            <Button variant="ghost" keepFocus onClick={() => setConfirmReset(false)}>
              Cancel
            </Button>
          </div>
        )}
        {keymap.warnings.length > 0 && (
          <ul className="flex flex-col gap-0.5 rounded-control border border-line bg-raised px-2 py-1.5 text-sm text-warning">
            {keymap.warnings.map((warning) => (
              <li key={warning} className="selectable">
                {warning}
              </li>
            ))}
          </ul>
        )}
      </div>

      {shown.map((group) => (
        <div key={group.id} className="flex flex-col">
          <h4 className="flex h-7 items-center gap-2 font-medium text-fg-3 text-xs uppercase tracking-[0.06em]">
            {group.label}
            {group.context && <Chip>only in the {CONTEXT_LABELS[group.context]} panel</Chip>}
          </h4>
          {group.rows.map((row) => {
            const id = rowId(row.context, row.actionId);
            const capturing =
              capture?.kind === "row" &&
              capture.context === row.context &&
              capture.actionId === row.actionId;
            const conflict =
              pending && pending.context === row.context && pending.actionId === row.actionId
                ? pending
                : null;
            return (
              <div key={id} data-hotkey-row={id} className="flex flex-col">
                <div className="group flex min-h-8 items-center gap-2 rounded-control px-1 hover:bg-hover">
                  <span
                    aria-hidden
                    className={`h-1.5 w-1.5 shrink-0 rounded-control ${row.modified ? "bg-accent" : ""}`}
                  />
                  <span className="min-w-0 flex-1 truncate text-base text-fg">{row.label}</span>
                  <div className="flex flex-wrap items-center justify-end gap-1">
                    {capturing ? (
                      <span
                        data-capture-target
                        role="status"
                        className="inline-flex h-6 items-center rounded-control border border-accent bg-accent-soft px-2 text-accent-fg text-sm"
                      >
                        Press a key… (Esc to cancel)
                      </span>
                    ) : (
                      row.keys.map((key) => (
                        <Cap
                          key={key}
                          binding={key}
                          label={row.label}
                          onEdit={() => startCapture(row, key)}
                          onRemove={() =>
                            void write(row.context, row.actionId, withoutKey(row.keys, key))
                          }
                        />
                      ))
                    )}
                  </div>
                  <span className="flex w-6 justify-center">
                    {row.modified && !capturing && (
                      <Tooltip
                        label={
                          row.defaults.length > 0
                            ? `Reset to ${row.defaults.map(displayBinding).join(", ")}`
                            : "Reset to unbound"
                        }
                      >
                        <button
                          type="button"
                          aria-label={`Reset ${row.label}`}
                          onClick={() => void write(row.context, row.actionId, row.defaults)}
                          className="flex h-6 w-6 items-center justify-center rounded-control text-fg-2 hover:bg-active hover:text-fg"
                        >
                          <RotateCcw size={14} strokeWidth={1.5} aria-hidden />
                        </button>
                      </Tooltip>
                    )}
                  </span>
                  <Tooltip label="Add a shortcut">
                    <button
                      type="button"
                      aria-label={`Add a shortcut for ${row.label}`}
                      onClick={() => (capturing ? setCapture(null) : startCapture(row))}
                      className="flex h-6 w-6 shrink-0 items-center justify-center rounded-control text-fg-2 hover:bg-active hover:text-fg"
                    >
                      <Plus size={14} strokeWidth={1.5} aria-hidden />
                    </button>
                  </Tooltip>
                </div>
                {conflict && (
                  <div
                    role="alert"
                    data-conflict
                    className="mb-1 ml-4 flex flex-wrap items-center gap-2 rounded-control border border-line-strong bg-raised px-2 py-1.5 text-base text-fg"
                  >
                    <span className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
                      <KeyCap binding={conflict.conflict.key} />
                      is used by
                      <strong className="font-medium">
                        {getAction(conflict.conflict.actionId)?.label ?? conflict.conflict.actionId}
                      </strong>
                      .
                    </span>
                    <Button variant="primary" keepFocus onClick={() => resolve("replace")}>
                      Replace
                    </Button>
                    <Button keepFocus onClick={() => resolve("neither")}>
                      Keep both off
                    </Button>
                    <Button variant="ghost" keepFocus onClick={() => setPending(null)}>
                      Cancel
                    </Button>
                  </div>
                )}
                {refusal?.row === id && (
                  <div
                    role="alert"
                    className="mb-1 ml-4 flex items-center gap-2 rounded-control border border-line bg-raised px-2 py-1.5 text-base text-warning"
                  >
                    <span className="min-w-0 flex-1">{refusal.text}</span>
                    <Button size="sm" variant="ghost" keepFocus onClick={() => setRefusal(null)}>
                      OK
                    </Button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      ))}

      {custom.length > 0 && (
        <div className="flex flex-col">
          <h4 className="flex h-7 items-center gap-2 font-medium text-fg-3 text-xs uppercase tracking-[0.06em]">
            Custom
            <Chip>made in keybinds.toml</Chip>
          </h4>
          {custom.map((binding) => (
            <div
              key={binding.index}
              data-custom-row={binding.index}
              className="flex min-h-8 items-center gap-2 rounded-control px-1 hover:bg-hover"
            >
              <span aria-hidden className="h-1.5 w-1.5 shrink-0" />
              <span className="min-w-0 flex-1 truncate text-base text-fg">
                {customLabel(binding)}
                {binding.label && (
                  <span className="ml-2 font-mono text-fg-3 text-xs">{binding.action}</span>
                )}
              </span>
              <div className="flex flex-wrap items-center justify-end gap-1">
                {binding.keys.map((key) => (
                  <KeyCap key={key} binding={key} className="h-6" />
                ))}
              </div>
              <span className="w-6" />
              <Tooltip label="Delete this custom binding">
                <button
                  type="button"
                  aria-label={`Delete ${customLabel(binding)}`}
                  onClick={() => void keybindsRemoveCustom(binding.index).catch(reportError)}
                  className="flex h-6 w-6 shrink-0 items-center justify-center rounded-control text-fg-2 hover:bg-danger-soft hover:text-danger"
                >
                  <X size={14} strokeWidth={1.5} aria-hidden />
                </button>
              </Tooltip>
            </div>
          ))}
        </div>
      )}

      {shown.length === 0 && custom.length === 0 && (
        <p className="py-6 text-center text-base text-fg-3">
          {foundKey ? "Nothing is bound to that key." : "No action matches."}
        </p>
      )}
    </section>
  );
}

/** A key cap that edits on click and removes with its ✕ or Backspace. */
function Cap({
  binding,
  label,
  onEdit,
  onRemove,
}: {
  binding: string;
  label: string;
  onEdit: () => void;
  onRemove: () => void;
}) {
  const shown = displayBinding(binding);
  return (
    <span className="group/cap relative inline-flex">
      <button
        type="button"
        aria-label={`${label}: ${shown}. Press to change, Backspace to remove.`}
        onClick={onEdit}
        onKeyDown={(e) => {
          if (e.key === "Backspace" || e.key === "Delete") {
            e.preventDefault();
            e.stopPropagation();
            onRemove();
          }
        }}
        className="inline-flex h-6 items-center rounded-control border border-line-strong bg-raised px-1.5 font-mono text-fg-2 text-xs leading-none hover:border-accent hover:text-fg"
      >
        {shown}
      </button>
      <button
        type="button"
        tabIndex={-1}
        aria-label={`Remove ${shown} from ${label}`}
        onClick={onRemove}
        className="absolute -top-1.5 -right-1.5 hidden h-3.5 w-3.5 items-center justify-center rounded-control border border-line-strong bg-active text-fg group-hover/cap:flex"
      >
        <X size={10} strokeWidth={2} aria-hidden />
      </button>
    </span>
  );
}
