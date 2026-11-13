/**
 * Small panel controls (DESIGN_SYSTEM §8.6): toggle, segmented control, select, chip, and the
 * section eyebrow used by the sidebar tabs. Pointer presses never leave focus on a control
 * (§13); keyboard use works as native buttons and radios do.
 */
import { Check, ChevronDown } from "lucide-react";
import { DropdownMenu as M } from "radix-ui";
import { type KeyboardEvent, type ReactNode, useRef } from "react";
import { Tooltip } from "./Tooltip";

const noFocusOnPress = (e: { preventDefault: () => void }) => e.preventDefault();

/** Section heading inside a panel: `text-xs` eyebrow with an optional right-hand slot. */
export function Section({
  title,
  right,
  children,
}: {
  title: string;
  right?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-2" aria-label={title}>
      <div className="flex h-6 items-center gap-2">
        <h3 className="flex-1 font-medium text-fg-3 text-xs uppercase tracking-[0.06em]">
          {title}
        </h3>
        {right}
      </div>
      {children}
    </section>
  );
}

/** 32 × 18 rectangular switch (not a pill). */
export function Toggle({
  checked,
  onChange,
  label,
  showLabel = true,
  hint,
  disabled,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  showLabel?: boolean;
  hint?: string;
  disabled?: boolean;
}) {
  const button = (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onMouseDown={noFocusOnPress}
      onClick={() => onChange(!checked)}
      className="group inline-flex shrink-0 items-center gap-2 rounded-control text-base text-fg-2 disabled:opacity-50"
    >
      {showLabel && <span className="group-hover:text-fg">{label}</span>}
      <span
        className={`relative inline-flex h-4.5 w-8 items-center rounded-control transition-colors duration-(--dur-fast) ${
          checked ? "bg-accent" : "bg-track"
        }`}
      >
        <span
          className={`absolute h-3.5 w-3.5 rounded-knob transition-[left] duration-(--dur-fast) ${
            checked ? "left-4 bg-on-accent" : "left-0.5 bg-fg-2"
          }`}
        />
      </span>
    </button>
  );
  return hint ? <Tooltip label={hint}>{button}</Tooltip> : button;
}

export interface Option<T extends string> {
  value: T;
  label: string;
  hint?: string;
}

/** 2–7 mutually exclusive options; arrow keys move between them (radio group). */
export function Segmented<T extends string>({
  options,
  value,
  onChange,
  label,
  fit = false,
  className = "",
}: {
  options: Option<T>[];
  value: T | null;
  onChange: (value: T) => void;
  label: string;
  /** Segments as wide as their labels instead of equal widths (long, uneven labels). */
  fit?: boolean;
  className?: string;
}) {
  const group = useRef<HTMLDivElement>(null);
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const dir = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (dir === 0) return;
    e.preventDefault();
    e.stopPropagation();
    const index = Math.max(
      0,
      options.findIndex((o) => o.value === value),
    );
    const next = options[(index + dir + options.length) % options.length];
    if (!next) return;
    onChange(next.value);
    const buttons = group.current?.querySelectorAll<HTMLButtonElement>("button");
    buttons?.[options.indexOf(next)]?.focus();
  };
  return (
    <div
      ref={group}
      role="radiogroup"
      aria-label={label}
      onKeyDown={onKeyDown}
      className={`flex rounded-control bg-raised p-0.5 ${className}`}
    >
      {options.map((option) => {
        const selected = option.value === value;
        const button = (
          // biome-ignore lint/a11y/useSemanticElements: a styled segment; a native radio input cannot carry this look
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected || (value === null && option === options[0]) ? 0 : -1}
            onMouseDown={noFocusOnPress}
            onClick={() => onChange(option.value)}
            className={`relative h-6 rounded-control px-1.5 text-sm ${
              fit ? "flex-auto whitespace-nowrap" : "min-w-0 flex-1 truncate"
            } ${
              selected
                ? "bg-active text-fg after:absolute after:inset-x-1.5 after:bottom-0 after:h-0.5 after:bg-accent"
                : "text-fg-2 hover:text-fg"
            }`}
          >
            {option.label}
          </button>
        );
        return option.hint ? (
          <Tooltip key={option.value} label={option.hint}>
            {button}
          </Tooltip>
        ) : (
          button
        );
      })}
    </div>
  );
}

/** A select: a raised button that opens a menu of choices (§8.6, §8.7). */
export function Select<T extends string>({
  options,
  value,
  onChange,
  label,
  placeholder = "None",
  className = "",
}: {
  options: Option<T>[];
  value: T | null;
  onChange: (value: T) => void;
  label: string;
  placeholder?: string;
  className?: string;
}) {
  const current = options.find((o) => o.value === value);
  return (
    <M.Root modal={false}>
      <M.Trigger asChild>
        <button
          type="button"
          aria-label={label}
          disabled={options.length === 0}
          className={`flex h-7 min-w-0 items-center gap-2 rounded-control border border-line bg-raised px-2 text-left text-base text-fg hover:border-line-strong disabled:text-fg-off ${className}`}
        >
          <span className="min-w-0 flex-1 truncate">{current?.label ?? placeholder}</span>
          <ChevronDown size={14} strokeWidth={1.5} className="shrink-0 text-fg-2" aria-hidden />
        </button>
      </M.Trigger>
      <M.Portal>
        <M.Content
          align="start"
          sideOffset={4}
          collisionPadding={8}
          className="z-(--z-floating) max-h-80 min-w-(--radix-dropdown-menu-trigger-width) overflow-y-auto rounded-surface border border-line-strong bg-raised p-1 shadow-float"
        >
          <M.RadioGroup value={value ?? ""} onValueChange={(v) => onChange(v as T)}>
            {options.map((option) => (
              <M.RadioItem
                key={option.value}
                value={option.value}
                className="flex h-7 cursor-default select-none items-center gap-2 rounded-control px-2 text-base text-fg outline-none data-[highlighted]:bg-hover"
              >
                <span className="flex w-4 justify-center">
                  <M.ItemIndicator>
                    <Check size={14} strokeWidth={1.5} className="text-accent-fg" aria-hidden />
                  </M.ItemIndicator>
                </span>
                <span className="flex-1 truncate">{option.label}</span>
                {option.hint && <span className="text-fg-3 text-sm">{option.hint}</span>}
              </M.RadioItem>
            ))}
          </M.RadioGroup>
        </M.Content>
      </M.Portal>
    </M.Root>
  );
}

/** 18 px badge; `accent` for the active/positive variant. */
export function Chip({ children, accent }: { children: ReactNode; accent?: boolean }) {
  return (
    <span
      className={`inline-flex h-4.5 shrink-0 items-center rounded-control px-1.5 text-xs ${
        accent ? "bg-accent-soft text-accent-fg" : "bg-active text-fg-2"
      }`}
    >
      {children}
    </span>
  );
}
