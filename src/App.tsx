/**
 * The window (DESIGN_SYSTEM §6): video shows through the transparent page; the interaction
 * layer, idle screen, OSD, chrome, menus, and toasts stack above it (§6.2).
 */
import { useEffect } from "react";
import { registerActions } from "./actions";
import { PLAYER_ROOT_ID } from "./actions/definitions/window";
import { TooltipProvider } from "./components/Tooltip";
import { Chrome } from "./features/chrome/Chrome";
import { ContextMenu } from "./features/chrome/ContextMenu";
import { DropHint, useFileDrop } from "./features/chrome/DropTarget";
import { InteractionLayer } from "./features/chrome/InteractionLayer";
import { OsdFlash, Toasts } from "./features/chrome/Osd";
import { ResizeZones } from "./features/chrome/ResizeZones";
import { useAutoHide } from "./features/chrome/useAutoHide";
import { useKeyboard } from "./features/chrome/useKeyboard";
import { DebugOverlay } from "./features/debug/DebugOverlay";
import { IdleScreen } from "./features/idle/IdleScreen";
import { appReady } from "./ipc/commands";
import { useAppStore } from "./stores/app";
import { reportError } from "./stores/osd";
import { startSync } from "./stores/sync";
import { useUiStore } from "./stores/ui";

registerActions();

export function App() {
  const info = useAppStore((s) => s.info);
  const cursorHidden = useUiStore((s) => s.cursorHidden);
  const zen = useUiStore((s) => s.zen);

  useEffect(() => startSync(), []);
  useKeyboard();
  useAutoHide();
  useFileDrop();

  // Default focus is the player root, so single-key shortcuts work at once (§13).
  useEffect(() => {
    document.getElementById(PLAYER_ROOT_ID)?.focus({ preventScroll: true });
  }, []);

  // The window stays hidden until the first frame is on screen (ARCHITECTURE §4.3).
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      appReady().catch(reportError);
    });
    return () => cancelAnimationFrame(frame);
  }, []);

  return (
    <TooltipProvider>
      <div
        id={PLAYER_ROOT_ID}
        tabIndex={-1}
        className={`relative h-full w-full overflow-hidden outline-none ${cursorHidden ? "cursor-none" : ""}`}
      >
        <InteractionLayer />
        <IdleScreen />
        {import.meta.env.DEV && !zen && <DebugOverlay info={info} />}
        <OsdFlash />
        <Chrome />
        <ContextMenu />
        <Toasts />
        <DropHint />
        <ResizeZones />
      </div>
    </TooltipProvider>
  );
}
