/**
 * Files dragged from the OS onto the window (DESIGN_SYSTEM §10.7): dropping on the video
 * replaces what is playing; dropping on the queue panel inserts at the line under the pointer.
 * A hint overlay says what a drop on the video will do. (A Shift-drop cannot be detected
 * during an OS drag in the webview, so "enqueue" is done by dropping on the queue.)
 */
import { useEffect } from "react";
import { playerOpen, queueAdd } from "@/ipc/commands";
import { onFileDrop } from "@/ipc/transport";
import { reportError } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";

export function useFileDrop(): void {
  useEffect(
    () =>
      onFileDrop((event) => {
        const ui = useUiStore.getState();
        const index = event.type === "leave" ? null : (ui.dropResolver?.(event.x, event.y) ?? null);
        if (event.type === "enter" || event.type === "over") {
          ui.setDropIndex(index);
          const hint = index === null ? "replace" : null;
          if (ui.dropHint !== hint) ui.setDropHint(hint);
          return;
        }
        ui.setDropHint(null);
        ui.setDropIndex(null);
        if (event.type === "drop" && event.paths.length > 0) {
          const done =
            index === null
              ? playerOpen({ sources: event.paths, mode: "replace" })
              : queueAdd(event.paths, index);
          void done.catch(reportError);
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
