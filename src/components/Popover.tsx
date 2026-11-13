/** Popovers (DESIGN_SYSTEM §8.7): raised surface, radius 6, strong border, elevation shadow. */
import { Popover as Radix } from "radix-ui";
import type { ReactNode } from "react";

export function Popover({
  trigger,
  children,
  open,
  onOpenChange,
  side = "top",
  align = "center",
  width = "w-70",
}: {
  trigger: ReactNode;
  children: ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  side?: "top" | "bottom" | "left" | "right";
  align?: "start" | "center" | "end";
  width?: string;
}) {
  return (
    <Radix.Root open={open} onOpenChange={onOpenChange}>
      <Radix.Trigger asChild>{trigger}</Radix.Trigger>
      <Radix.Portal>
        <Radix.Content
          side={side}
          align={align}
          sideOffset={8}
          collisionPadding={8}
          className={`z-(--z-floating) max-h-90 ${width} overflow-y-auto rounded-surface border border-line-strong bg-raised p-1 shadow-float`}
        >
          {children}
        </Radix.Content>
      </Radix.Portal>
    </Radix.Root>
  );
}
