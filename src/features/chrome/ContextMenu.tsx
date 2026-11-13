/**
 * The context menu (DESIGN_SYSTEM §8.8). A dropdown anchored to an invisible point, so it opens
 * the same way from a right-click, the ContextMenu key, or Shift+F10.
 */
import { useEffect } from "react";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { DropdownMenu, MenuContent } from "@/components/Menu";
import { useUiStore } from "@/stores/ui";
import { contextMenuEntries } from "./menus";

export function ContextMenu() {
  const at = useUiStore((s) => s.contextMenu);
  const close = useUiStore((s) => s.closeContextMenu);
  const hold = useUiStore((s) => s.hold);
  const open = at !== null;
  // The chrome stays up while the menu is open (§7.1 "Held").
  useEffect(() => {
    hold("menu-context", open);
    return () => hold("menu-context", false);
  }, [open, hold]);
  if (!at) return null;
  return (
    <DropdownMenu.Root
      open
      modal={false}
      onOpenChange={(next) => {
        if (!next) close();
      }}
    >
      <DropdownMenu.Trigger asChild>
        <div
          aria-hidden="true"
          className="pointer-events-none fixed h-0 w-0"
          style={{ left: at.x, top: at.y }}
        />
      </DropdownMenu.Trigger>
      <MenuContent
        entries={contextMenuEntries()}
        align="start"
        side="bottom"
        onCloseAutoFocus={(e) => {
          e.preventDefault();
          focusPlayerRoot();
        }}
      />
    </DropdownMenu.Root>
  );
}
