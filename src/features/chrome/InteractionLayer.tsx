/**
 * The transparent full-window layer over the video (DESIGN_SYSTEM §6.2, z 0). Mouse and wheel
 * bindings apply only here (KEYBINDS_AND_CONFIG §3.1): click, double-click, middle, back,
 * forward, wheel, right-click. Dragging past a small threshold moves the window instead
 * (`controls.drag_moves_window`), and that click does not count.
 */
import { type MouseEvent, type PointerEvent, useRef, type WheelEvent } from "react";
import { dispatch } from "@/actions";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { startWindowDrag } from "@/ipc/transport";
import { resolveBinding } from "@/keybinds/engine";
import { bindingFromMouse, bindingFromWheel } from "@/keybinds/normalize";
import { useConfigStore } from "@/stores/config";
import { useKeymapStore } from "@/stores/keymap";
import { reportError } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";

const DRAG_THRESHOLD_PX = 4;

function fire(binding: string | null): boolean {
  if (!binding) return false;
  const { lookup } = useKeymapStore.getState();
  const match = resolveBinding(lookup, binding, {
    context: "global",
    inTextInput: false,
    focusedControlKeys: new Set(),
  });
  if (!match) return false;
  if (match.kind === "custom") {
    void dispatch(match.binding.action, { trigger: "mouse", binding, args: match.binding.args });
  } else {
    void dispatch(match.actionId, { trigger: "mouse", binding });
  }
  return true;
}

export function InteractionLayer() {
  const down = useRef<{ x: number; y: number; dragged: boolean } | null>(null);
  const suppressClick = useRef(false);

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    focusPlayerRoot();
    if (e.button === 0) {
      down.current = { x: e.clientX, y: e.clientY, dragged: false };
      suppressClick.current = false;
    }
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const start = down.current;
    if (!start || start.dragged || (e.buttons & 1) === 0) return;
    const config = useConfigStore.getState().config;
    const fullscreen = useUiStore.getState().window.mode === "fullscreen";
    if (!(config?.controls.drag_moves_window ?? true) || fullscreen) return;
    if (Math.hypot(e.clientX - start.x, e.clientY - start.y) < DRAG_THRESHOLD_PX) return;
    start.dragged = true;
    suppressClick.current = true;
    void startWindowDrag().catch(reportError);
  };

  const onPointerUp = () => {
    down.current = null;
  };

  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    // Fires immediately: a double-click toggles twice (net zero), then runs MouseLeftDouble.
    fire(bindingFromMouse(e));
  };

  const onDoubleClick = (e: MouseEvent<HTMLDivElement>) => {
    if (suppressClick.current) return;
    fire(bindingFromMouse(e, true));
  };

  const onAuxClick = (e: MouseEvent<HTMLDivElement>) => {
    // Middle, back, forward. (Right-click goes through onContextMenu.)
    if (e.button === 2) return;
    if (fire(bindingFromMouse(e))) e.preventDefault();
  };

  const onMouseDown = (e: MouseEvent<HTMLDivElement>) => {
    // Stop the browser's own back/forward and middle-click autoscroll.
    if (e.button === 1 || e.button === 3 || e.button === 4) e.preventDefault();
  };

  const onContextMenu = (e: MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    useUiStore.getState().setPointer(e.clientX, e.clientY);
    fire(bindingFromMouse(e));
  };

  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    const invert = useConfigStore.getState().config?.controls.wheel_invert ?? false;
    fire(bindingFromWheel(e, invert));
  };

  return (
    <div
      aria-hidden="true"
      className="absolute inset-0 z-(--z-interaction)"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onMouseDown={onMouseDown}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onAuxClick={onAuxClick}
      onContextMenu={onContextMenu}
      onWheel={onWheel}
    />
  );
}
