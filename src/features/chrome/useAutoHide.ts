/** Wires the `AutoHide` controller to the stores and to pointer movement. */
import { useEffect, useRef } from "react";
import { useConfig } from "@/stores/config";
import { usePlayer } from "@/stores/player";
import { isHeld, useUiStore } from "@/stores/ui";
import { AutoHide } from "./autoHide";

export function useAutoHide(): void {
  const delay = useConfig((c) => c.interface.autohide_delay_ms) ?? 2000;
  const autohideWindowed = useConfig((c) => c.interface.autohide_in_windowed) ?? true;
  const playing = usePlayer((s) => s.status === "playing") ?? false;
  const fullscreen = useUiStore((s) => s.window.mode === "fullscreen");
  const zen = useUiStore((s) => s.zen);
  const holds = useUiStore((s) => s.holds);
  const revealRequest = useUiStore((s) => s.revealRequest);
  const controller = useRef<AutoHide | null>(null);

  useEffect(() => {
    // Lives for the app's lifetime; the delay is pushed in by the next effect.
    const ctl = new AutoHide(2000, (v) =>
      useUiStore
        .getState()
        .setVisibility({ bottom: v.bottom, top: v.top, cursorHidden: v.cursorHidden }),
    );
    controller.current = ctl;
    const onMove = (e: PointerEvent) => {
      useUiStore.getState().setPointer(e.clientX, e.clientY);
      ctl.pointerMove(e.clientY);
    };
    const onLeave = () => ctl.pointerLeave();
    window.addEventListener("pointermove", onMove);
    document.documentElement.addEventListener("pointerleave", onLeave);
    return () => {
      window.removeEventListener("pointermove", onMove);
      document.documentElement.removeEventListener("pointerleave", onLeave);
      ctl.dispose();
      controller.current = null;
    };
  }, []);

  useEffect(() => {
    controller.current?.setDelay(delay);
  }, [delay]);

  useEffect(() => {
    controller.current?.setInput({
      playing,
      fullscreen,
      autohideWindowed,
      held: isHeld(holds),
      zen,
      topFocused: Boolean(holds["focus-top"]),
    });
  }, [playing, fullscreen, autohideWindowed, holds, zen]);

  useEffect(() => {
    if (revealRequest > 0) controller.current?.reveal();
  }, [revealRequest]);
}
