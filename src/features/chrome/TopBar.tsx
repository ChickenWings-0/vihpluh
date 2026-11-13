/**
 * Top bar (DESIGN_SYSTEM §7.3): title button (opens the Library drawer, Phase 9), the single
 * consolidated ☰ menu, PiP, and custom window controls. Everything that is not a button is a
 * drag region; double-click toggles maximize.
 */
import { ChevronRight, Copy, Menu as MenuIcon, Minus, Square, X } from "lucide-react";
import { type PointerEvent, useState } from "react";
import { dispatch, isEnabled } from "@/actions";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { IconButton } from "@/components/IconButton";
import { DropdownMenu, MenuContent } from "@/components/Menu";
import { Tooltip } from "@/components/Tooltip";
import { PipGlyph } from "@/icons/mono";
import { windowClose, windowMinimize, windowToggleMaximize } from "@/ipc/commands";
import { startWindowDrag } from "@/ipc/transport";
import { useConfig } from "@/stores/config";
import { useShortcut } from "@/stores/keymap";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { useUiStore } from "@/stores/ui";
import { appMenuEntries } from "./menus";

function folderOf(source: string): string {
  const parts = source.split(/[\\/]/).filter(Boolean);
  return parts.length > 1 ? (parts[parts.length - 2] ?? "") : "";
}

function TitleButton() {
  const title = usePlayer((s) => s.media?.title ?? null);
  const source = usePlayer((s) => s.media?.source ?? null);
  const isUrl = usePlayer((s) => s.media?.isUrl ?? false);
  const shortcut = useShortcut("ui.toggle_library");
  const secondary = source && !isUrl ? folderOf(source) : null;
  const enabled = isEnabled("ui.toggle_library");
  return (
    <Tooltip
      label={enabled ? "Library" : "Library — coming in Phase 9"}
      shortcut={shortcut}
      side="bottom"
    >
      <button
        type="button"
        onClick={(e) => {
          void dispatch("ui.toggle_library", { trigger: "button" });
          if (e.detail > 0) focusPlayerRoot();
        }}
        className="flex min-w-0 items-center gap-1.5 rounded-control px-2 py-1 hover:bg-hover"
      >
        <ChevronRight size={16} strokeWidth={1.5} className="shrink-0 text-fg-2" aria-hidden />
        <span className="truncate font-medium text-fg text-md">{title ?? "Library"}</span>
        {secondary && <span className="truncate text-base text-fg-2">{secondary}</span>}
      </button>
    </Tooltip>
  );
}

function AppMenu() {
  const hold = useUiStore((s) => s.hold);
  const [open, setOpen] = useState(false);
  return (
    <DropdownMenu.Root
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        hold("menu-app", next);
      }}
    >
      <DropdownMenu.Trigger asChild>
        <IconButton
          icon={MenuIcon}
          label="Menu"
          tooltipSide="bottom"
          on={open}
          keepFocus
          onClick={() => {}}
        />
      </DropdownMenu.Trigger>
      {open && (
        <MenuContent
          entries={appMenuEntries()}
          align="end"
          onCloseAutoFocus={(e) => {
            e.preventDefault();
            focusPlayerRoot();
          }}
        />
      )}
    </DropdownMenu.Root>
  );
}

function WindowControls() {
  const maximized = useUiStore((s) => s.window.maximized);
  return (
    <div className="flex h-full items-stretch">
      <IconButton
        icon={Minus}
        label="Minimize"
        size="window"
        tooltipSide="bottom"
        className="rounded-none"
        onClick={() => void windowMinimize().catch(reportError)}
      />
      <IconButton
        icon={maximized ? Copy : Square}
        label={maximized ? "Restore" : "Maximize"}
        size="window"
        tooltipSide="bottom"
        className="rounded-none"
        onClick={() => void windowToggleMaximize().catch(reportError)}
      />
      <IconButton
        icon={X}
        label="Close"
        size="window"
        tooltipSide="bottom"
        className="rounded-none"
        danger
        onClick={() => void windowClose().catch(reportError)}
      />
    </div>
  );
}

export function TopBar() {
  const fullscreen = useUiStore((s) => s.window.mode === "fullscreen");
  const native = useConfig((c) => c.interface.window_decorations === "native") ?? false;

  // Drag from anywhere that is not a control; double-click maximizes.
  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || fullscreen) return;
    if ((e.target as HTMLElement).closest("button, [role=slider], a, input")) return;
    if (e.detail === 2) {
      void windowToggleMaximize().catch(reportError);
      return;
    }
    void startWindowDrag().catch(reportError);
  };

  return (
    <div onPointerDown={onPointerDown} className="flex h-10 w-full items-center gap-2 pl-2">
      <TitleButton />
      <div className="flex-1" />
      <div className="flex shrink-0 items-center gap-1 pr-1">
        <AppMenu />
        <IconButton
          icon={PipGlyph}
          label="Picture-in-picture"
          actionId="window.toggle_pip"
          tooltipSide="bottom"
        />
      </div>
      {!fullscreen && !native && <WindowControls />}
    </div>
  );
}
