/**
 * Tooltips (DESIGN_SYSTEM §8.10): 500 ms delay, instant between adjacent controls, raised
 * surface, text-sm, max 240 px, `Label [key cap]`. Prefers the side away from the video.
 */
import { Tooltip as Radix } from "radix-ui";
import type { ReactNode } from "react";
import { useConfig } from "@/stores/config";
import { KeyCap } from "./KeyCap";

export function TooltipProvider({ children }: { children: ReactNode }) {
  return (
    <Radix.Provider delayDuration={500} skipDelayDuration={300}>
      {children}
    </Radix.Provider>
  );
}

export function Tooltip({
  label,
  shortcut,
  side = "top",
  children,
}: {
  label: string;
  shortcut?: string;
  side?: "top" | "bottom" | "left" | "right";
  children: ReactNode;
}) {
  const showShortcut = useConfig((c) => c.interface.tooltip_shortcuts) ?? true;
  return (
    <Radix.Root>
      <Radix.Trigger asChild>{children}</Radix.Trigger>
      <Radix.Portal>
        <Radix.Content
          side={side}
          sideOffset={6}
          collisionPadding={8}
          className="z-(--z-tooltip) flex max-w-60 items-center gap-2 rounded-control border border-line-strong bg-raised px-2 py-1 text-fg text-sm shadow-float"
        >
          <span>{label}</span>
          {showShortcut && shortcut && <KeyCap binding={shortcut} />}
        </Radix.Content>
      </Radix.Portal>
    </Radix.Root>
  );
}
