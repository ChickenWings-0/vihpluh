/**
 * Files dragged from the OS onto the window (DESIGN_SYSTEM §10.7): dropping on the video
 * replaces what is playing. Shift-drop to enqueue arrives with the queue in Phase 4. A hint
 * overlay says what will happen.
 */
import { useEffect } from "react";
import { playerOpen } from "@/ipc/commands";
import { onFileDrop } from "@/ipc/transport";
import { reportError } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";

export function useFileDrop(): void {
  useEffect(
    () =>
      onFileDrop((event) => {
        const ui = useUiStore.getState();
        if (event.type === "enter" || event.type === "over") {
          if (ui.dropHint !== "replace") ui.setDropHint("replace");
          return;
        }
        ui.setDropHint(null);
        if (event.type === "drop" && event.paths.length > 0) {
          void playerOpen({ sources: event.paths, mode: "replace" }).catch(reportError);
        }
      }),
    [],
  );
}

export function DropHint() {
  const hint = useUiStore((s) => s.dropHint);
  if (!hint) return null;
  return (
    <div className="pointer-events-none absolute inset-3 z-(--z-overlay) flex items-center justify-center rounded-surface border-2 border-accent border-dashed bg-scrim">
      <span className="rounded-surface bg-raised px-4 py-2 font-semibold text-fg text-lg shadow-float">
        {hint === "replace" ? "Drop to play" : "Drop to add to the queue"}
      </span>
    </div>
  );
}
