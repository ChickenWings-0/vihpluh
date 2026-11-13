/**
 * Library, playlist, and stream actions (KEYBINDS_AND_CONFIG §2, Phase 9). The drawer's rows,
 * buttons, and menus dispatch the same actions as the `library` context's shortcuts.
 */
import { CornerLeftUp, Library, Link, ListPlus, ListVideo, Play, Save, Search } from "lucide-react";
import { playFrom, type Row, resolveSources } from "@/features/library/model";
import type { OpenMode } from "@/ipc/bindings/OpenMode";
import { playerOpen, queueSaveAsPlaylist } from "@/ipc/commands";
import { selectedRow, useLibraryStore } from "@/stores/library";
import { useOsdStore } from "@/stores/osd";
import { useQueueStore } from "@/stores/queue";
import { useUiStore } from "@/stores/ui";
import type { Action, Feedback } from "../types";

const VERB: Record<OpenMode, string> = {
  replace: "Playing",
  play_next: "Plays next",
  enqueue: "Added to queue",
};

/**
 * Plays a drawer row: now (replacing the queue), next, or at the end of the queue. The
 * drawer stays open — browsing never interrupts what is playing unless asked to.
 */
export async function playRow(row: Row, mode: OpenMode): Promise<Feedback | undefined> {
  const { rows, trail } = useLibraryStore.getState();
  // "Play now" inside an album or a season plays on from that row.
  const sources =
    (mode === "replace" ? playFrom(rows, row, trail) : null) ?? (await resolveSources(row));
  if (sources.length === 0) {
    useOsdStore.getState().pushToast({ kind: "warn", text: `Nothing to play in "${row.title}"` });
    return undefined;
  }
  await playerOpen({ sources, mode });
  const count = sources.length > 1 ? ` (${sources.length} items)` : "";
  if (mode !== "replace") {
    useOsdStore.getState().pushToast({
      kind: "success",
      text: `${VERB[mode]}: ${row.title}${count}`,
      group: "library",
    });
  }
  return { icon: Play, label: VERB[mode], value: row.title };
}

const inLibrary = () => useUiStore.getState().library;
const hasSelection = () => inLibrary() && selectedRow() !== null;

const playSelected = (mode: OpenMode) => async () => {
  const row = selectedRow();
  if (!row) return undefined;
  // Enter on something that opens (and plays nothing by itself) opens it.
  if (mode === "replace" && row.open && !row.sources) {
    useLibraryStore.getState().open(row.open);
    return undefined;
  }
  return playRow(row, mode);
};

/** A URL or a path, as typed or pasted. */
export function cleanSource(text: string): string | null {
  const value = text
    .trim()
    .replace(/^["']|["']$/g, "")
    .trim();
  return value === "" ? null : value;
}

export const libraryActions: Action[] = [
  {
    id: "ui.toggle_library",
    label: "Library",
    group: "window",
    icon: Library,
    isOn: inLibrary,
    run: () => {
      const ui = useUiStore.getState();
      ui.setLibrary(!ui.library);
      return undefined;
    },
  },
  {
    id: "app.open_url",
    label: "Open URL…",
    group: "app",
    icon: Link,
    run: () => {
      useUiStore.getState().ask({
        title: "Open URL",
        label: "Address of a stream, a video page, or a file",
        placeholder: "https://…",
        confirm: "Play",
        onSubmit: async (text) => {
          const source = cleanSource(text);
          if (source) await playerOpen({ sources: [source], mode: "replace" });
        },
      });
      return undefined;
    },
  },
  {
    id: "queue.save_as_playlist",
    label: "Save queue as playlist…",
    group: "queue",
    icon: Save,
    isEnabled: () => (useQueueStore.getState().queue?.items.length ?? 0) > 0,
    run: () => {
      useUiStore.getState().ask({
        title: "Save queue as playlist",
        label: "Playlist name",
        confirm: "Save",
        onSubmit: async (name) => {
          if (name.trim() === "") return;
          await queueSaveAsPlaylist(name.trim());
          useOsdStore
            .getState()
            .pushToast({ kind: "success", text: `Playlist "${name.trim()}" saved` });
          const library = useLibraryStore.getState();
          if (library.section === "playlists") library.reload();
        },
      });
      return undefined;
    },
  },
  {
    id: "library.play_selected",
    label: "Play now",
    group: "context",
    icon: Play,
    isEnabled: hasSelection,
    run: playSelected("replace"),
  },
  {
    id: "library.play_next_selected",
    label: "Play next",
    group: "context",
    icon: ListVideo,
    isEnabled: hasSelection,
    run: playSelected("play_next"),
  },
  {
    id: "library.enqueue_selected",
    label: "Add to queue",
    group: "context",
    icon: ListPlus,
    isEnabled: hasSelection,
    run: playSelected("enqueue"),
  },
  {
    id: "library.focus_search",
    label: "Search",
    group: "context",
    icon: Search,
    isEnabled: inLibrary,
    run: () => {
      useLibraryStore.getState().requestSearchFocus();
      return undefined;
    },
  },
  {
    id: "library.go_up",
    label: "Up one level",
    group: "context",
    icon: CornerLeftUp,
    isEnabled: () => inLibrary() && useLibraryStore.getState().trail.length > 0,
    run: () => {
      useLibraryStore.getState().up();
      return undefined;
    },
  },
];
