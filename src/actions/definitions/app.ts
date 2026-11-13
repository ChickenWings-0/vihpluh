/** App-level actions: opening media and quitting (KEYBINDS_AND_CONFIG §2). */
import { Clipboard, FileVideo, FolderOpen, Power } from "lucide-react";
import { appPickMedia, appQuit, playerOpen } from "@/ipc/commands";
import { useOsdStore } from "@/stores/osd";
import type { Action } from "../types";

/** Strips surrounding quotes and whitespace from a pasted path ("Copy as path" adds quotes). */
export function cleanPastedSource(text: string): string | null {
  const line = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .find((l) => l.length > 0);
  if (!line) return null;
  const unquoted = line.replace(/^["']|["']$/g, "").trim();
  return unquoted.length > 0 ? unquoted : null;
}

async function openPicked(kind: "files" | "folder") {
  const sources = await appPickMedia(kind);
  if (sources.length > 0) await playerOpen({ sources, mode: "replace" });
  return undefined;
}

export const appActions: Action[] = [
  {
    id: "app.open_file",
    label: "Open file…",
    group: "app",
    icon: FileVideo,
    run: () => openPicked("files"),
  },
  {
    id: "app.open_folder",
    label: "Open folder…",
    group: "app",
    icon: FolderOpen,
    run: () => openPicked("folder"),
  },
  {
    id: "app.paste_and_play",
    label: "Play path or URL from clipboard",
    group: "app",
    icon: Clipboard,
    run: async () => {
      let text = "";
      try {
        text = await navigator.clipboard.readText();
      } catch {
        useOsdStore
          .getState()
          .pushToast({ kind: "warn", text: "The clipboard could not be read." });
        return undefined;
      }
      const source = cleanPastedSource(text);
      if (!source) {
        useOsdStore
          .getState()
          .pushToast({ kind: "info", text: "The clipboard has no path or URL." });
        return undefined;
      }
      await playerOpen({ sources: [source], mode: "replace" });
      return undefined;
    },
  },
  {
    id: "app.quit",
    label: "Quit",
    group: "app",
    icon: Power,
    run: async () => {
      await appQuit();
      return undefined;
    },
  },
];
