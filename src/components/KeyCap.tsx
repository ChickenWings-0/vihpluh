import { displayBinding } from "@/keybinds/normalize";

/** A key cap (DESIGN_SYSTEM §8.6): mono text-xs, raised, 1 px strong border, radius 4. */
export function KeyCap({ binding, className = "" }: { binding: string; className?: string }) {
  return (
    <kbd
      className={`inline-flex h-4.5 items-center rounded-control border border-line-strong bg-raised px-1.5 font-mono text-fg-2 text-xs leading-none ${className}`}
    >
      {displayBinding(binding)}
    </kbd>
  );
}
