/**
 * Menus (DESIGN_SYSTEM §8.7): raised surface, radius 6, 28 px items with an icon slot, label,
 * live shortcut, and submenu chevron. One entry model feeds the ☰ menu, the context menu, and
 * any other dropdown, so every item that maps to an action shows that action's shortcut.
 */
import { Check, ChevronRight } from "lucide-react";
import { DropdownMenu as M } from "radix-ui";
import type { ComponentType, ReactNode } from "react";
import { dispatch, getAction, isEnabled } from "@/actions";
import { useKeymapStore } from "@/stores/keymap";
import { KeyCap } from "./KeyCap";

type Glyph = ComponentType<{ size?: number; strokeWidth?: number; "aria-hidden"?: boolean }>;

export type MenuEntry =
  | { kind: "action"; actionId: string; label?: string }
  | {
      kind: "item";
      label: string;
      onSelect: () => void;
      checked?: boolean;
      disabled?: boolean;
      hint?: string;
      icon?: Glyph;
    }
  | { kind: "separator" }
  | { kind: "label"; label: string }
  | { kind: "submenu"; label: string; icon?: Glyph; entries: MenuEntry[]; disabled?: boolean };

export const separator: MenuEntry = { kind: "separator" };
export const action = (actionId: string, label?: string): MenuEntry => ({
  kind: "action",
  actionId,
  label,
});

const ITEM =
  "flex h-7 cursor-default select-none items-center gap-2 rounded-control px-2 text-base text-fg outline-none data-[disabled]:text-fg-off data-[highlighted]:bg-hover";
const SURFACE =
  "z-(--z-floating) min-w-56 rounded-surface border border-line-strong bg-raised p-1 shadow-float";

function Row({
  icon: Icon,
  checked,
  label,
  right,
}: {
  icon?: Glyph;
  checked?: boolean;
  label: string;
  right?: ReactNode;
}) {
  return (
    <>
      <span className="flex w-4 shrink-0 items-center justify-center text-fg-2">
        {checked ? (
          <Check size={16} strokeWidth={1.5} className="text-accent-fg" aria-hidden />
        ) : Icon ? (
          <Icon size={16} strokeWidth={1.5} aria-hidden />
        ) : null}
      </span>
      <span className="flex-1 truncate">{label}</span>
      {right}
    </>
  );
}

function Entries({ entries }: { entries: MenuEntry[] }) {
  const keysOf = useKeymapStore((s) => s.lookup.keysOf);
  return (
    <>
      {entries.map((entry, index) => {
        const key = `${entry.kind}-${index}`;
        switch (entry.kind) {
          case "separator":
            return <M.Separator key={key} className="mx-1 my-1 h-px bg-line" />;
          case "label":
            return (
              <M.Label
                key={key}
                className="px-2 pt-2 pb-1 font-medium text-fg-3 text-xs uppercase tracking-[0.06em]"
              >
                {entry.label}
              </M.Label>
            );
          case "action": {
            const def = getAction(entry.actionId);
            if (!def) return null;
            const shortcut = keysOf.get(entry.actionId)?.[0];
            const checked = def.isOn ? def.isOn() : undefined;
            return (
              <M.Item
                key={key}
                className={ITEM}
                disabled={!isEnabled(entry.actionId)}
                onSelect={() => void dispatch(entry.actionId, { trigger: "menu" })}
              >
                <Row
                  icon={def.icon}
                  checked={checked}
                  label={entry.label ?? def.label}
                  right={shortcut ? <KeyCap binding={shortcut} /> : null}
                />
              </M.Item>
            );
          }
          case "item":
            return (
              <M.Item
                key={key}
                className={ITEM}
                disabled={entry.disabled}
                onSelect={entry.onSelect}
              >
                <Row
                  icon={entry.icon}
                  checked={entry.checked}
                  label={entry.label}
                  right={
                    entry.hint ? <span className="text-fg-3 text-sm">{entry.hint}</span> : null
                  }
                />
              </M.Item>
            );
          case "submenu":
            return (
              <M.Sub key={key}>
                <M.SubTrigger
                  className={ITEM}
                  disabled={entry.disabled || entry.entries.length === 0}
                >
                  <Row
                    icon={entry.icon}
                    label={entry.label}
                    right={<ChevronRight size={14} className="text-fg-3" aria-hidden />}
                  />
                </M.SubTrigger>
                <M.Portal>
                  <M.SubContent className={`${SURFACE} max-h-96 overflow-y-auto`} sideOffset={2}>
                    <Entries entries={entry.entries} />
                  </M.SubContent>
                </M.Portal>
              </M.Sub>
            );
        }
        return null;
      })}
    </>
  );
}

/** The menu surface. Render inside a `DropdownMenu.Root` with a trigger. */
export function MenuContent({
  entries,
  align = "start",
  side = "bottom",
  onCloseAutoFocus,
}: {
  entries: MenuEntry[];
  align?: "start" | "center" | "end";
  side?: "top" | "bottom" | "left" | "right";
  onCloseAutoFocus?: (e: Event) => void;
}) {
  return (
    <M.Portal>
      <M.Content
        className={SURFACE}
        align={align}
        side={side}
        sideOffset={4}
        collisionPadding={8}
        onCloseAutoFocus={onCloseAutoFocus}
      >
        <Entries entries={entries} />
      </M.Content>
    </M.Portal>
  );
}

export { M as DropdownMenu };
