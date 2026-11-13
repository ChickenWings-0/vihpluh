/**
 * Resize hit zones for the undecorated window (DESIGN_SYSTEM §9 "Normal"): 6 px edges, 12 px
 * corners. On Windows the toolkit already provides native resize borders, so these zones are
 * only drawn where it does not.
 */
import type { PointerEvent } from "react";
import { type ResizeEdge, startWindowResize } from "@/ipc/transport";
import { useAppStore } from "@/stores/app";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";

const ZONES: { edge: ResizeEdge; className: string }[] = [
  { edge: "North", className: "top-0 inset-x-3 h-1.5 cursor-ns-resize" },
  { edge: "South", className: "bottom-0 inset-x-3 h-1.5 cursor-ns-resize" },
  { edge: "West", className: "left-0 inset-y-3 w-1.5 cursor-ew-resize" },
  { edge: "East", className: "right-0 inset-y-3 w-1.5 cursor-ew-resize" },
  { edge: "NorthWest", className: "top-0 left-0 h-3 w-3 cursor-nwse-resize" },
  { edge: "NorthEast", className: "top-0 right-0 h-3 w-3 cursor-nesw-resize" },
  { edge: "SouthWest", className: "bottom-0 left-0 h-3 w-3 cursor-nesw-resize" },
  { edge: "SouthEast", className: "bottom-0 right-0 h-3 w-3 cursor-nwse-resize" },
];

export function ResizeZones() {
  const session = useAppStore((s) => s.info?.capabilities.session);
  const native = useConfig((c) => c.interface.window_decorations === "native") ?? false;
  const mode = useUiStore((s) => s.window.mode);
  const maximized = useUiStore((s) => s.window.maximized);
  // Windows: native resize borders already exist around the undecorated window.
  if (!session || session === "windows" || native || mode !== "normal" || maximized) return null;
  const start = (edge: ResizeEdge) => (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    void startWindowResize(edge).catch(reportError);
  };
  return (
    <>
      {ZONES.map(({ edge, className }) => (
        <div
          key={edge}
          aria-hidden="true"
          onPointerDown={start(edge)}
          className={`absolute z-(--z-tooltip) ${className}`}
        />
      ))}
    </>
  );
}
