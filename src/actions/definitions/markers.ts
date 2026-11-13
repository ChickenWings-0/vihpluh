/**
 * Bookmarks, chapter/bookmark navigation, and preset actions (KEYBINDS_AND_CONFIG §2,
 * Phase 8). Panels and shortcuts dispatch these same actions.
 */
import {
  Bookmark,
  BookmarkPlus,
  Layers,
  Pencil,
  Save,
  SkipBack,
  SkipForward,
  Trash2,
} from "lucide-react";
import {
  bookmarkAdd,
  bookmarkRemove,
  playerSeek,
  presetApply,
  presetUpdateCurrent,
} from "@/ipc/commands";
import { mergeMarkers, stepMarker } from "@/lib/markers";
import { formatTime } from "@/lib/time";
import { useBookmarkStore } from "@/stores/bookmarks";
import { useOsdStore } from "@/stores/osd";
import { usePresetStore } from "@/stores/presets";
import { getPosition } from "@/stores/time";
import { useUiStore } from "@/stores/ui";
import { config, hasMedia, player } from "../helpers";
import type { Action, Feedback, RunContext } from "../types";

const bookmarks = () => useBookmarkStore.getState();
const hasKey = () => (player()?.media?.key ?? "") !== "";

/** Opens the Bookmarks tab with a row in rename mode. */
function startRename(id: number): void {
  useUiStore.getState().showSidebarTab("bookmarks");
  bookmarks().rename(id);
}

/** Deletes a bookmark and offers to put it back (DESIGN_SYSTEM §10.5). */
export async function removeBookmark(id: number): Promise<void> {
  const removed = bookmarks().bookmarks.find((b) => b.id === id);
  await bookmarkRemove(id);
  if (!removed) return;
  useOsdStore.getState().pushToast({
    kind: "info",
    text: `Deleted "${removed.label}"`,
    action: { label: "Undo", run: () => void bookmarkAdd(removed.position, removed.label) },
    group: "bookmark-removed",
  });
}

async function stepTo(direction: 1 | -1): Promise<Feedback | undefined> {
  const state = player();
  if (!state) return undefined;
  const markers = mergeMarkers(state.chapters, bookmarks().bookmarks);
  const target = stepMarker(markers, getPosition(), direction);
  if (!target) return undefined;
  const exact = config()?.controls.precise_seek ?? true;
  await playerSeek({ kind: "absolute", value: target.time, exact });
  return {
    icon: direction === 1 ? SkipForward : SkipBack,
    label: target.label,
    value: formatTime(target.time),
  };
}

const hasMarkers = () =>
  hasMedia() && (player()?.chapters.length ?? 0) + bookmarks().bookmarks.length > 0;

export const markerActions: Action[] = [
  {
    id: "bookmarks.add",
    label: "Add bookmark here",
    group: "queue",
    icon: BookmarkPlus,
    isEnabled: hasKey,
    run: async (ctx: RunContext) => {
      // `{ seconds }` from the seek bar's menu; otherwise the current position.
      const at = ctx.args?.seconds === undefined ? undefined : Number(ctx.args.seconds);
      const bookmark = await bookmarkAdd(at);
      useOsdStore.getState().pushToast({
        kind: "success",
        text: `Bookmark added at ${formatTime(bookmark.position)}`,
        action: { label: "Rename", run: () => startRename(bookmark.id) },
        group: "bookmark-added",
      });
      return { icon: BookmarkPlus, label: bookmark.label, value: formatTime(bookmark.position) };
    },
  },
  {
    id: "bookmarks.toggle_panel",
    label: "Show bookmarks and chapters",
    group: "queue",
    icon: Bookmark,
    isOn: () => {
      const ui = useUiStore.getState();
      return ui.dock === "sidebar" && ui.sidebarTab === "bookmarks";
    },
    run: () => {
      useUiStore.getState().showSidebarTab("bookmarks", true);
      return undefined;
    },
  },
  {
    id: "ui.sidebar_bookmarks",
    label: "Bookmarks panel",
    group: "window",
    icon: Bookmark,
    isOn: () => {
      const ui = useUiStore.getState();
      return ui.dock === "sidebar" && ui.sidebarTab === "bookmarks";
    },
    run: () => {
      useUiStore.getState().showSidebarTab("bookmarks", true);
      return undefined;
    },
  },
  {
    id: "markers.next",
    label: "Next chapter or bookmark",
    group: "queue",
    icon: SkipForward,
    isEnabled: hasMarkers,
    run: () => stepTo(1),
  },
  {
    id: "markers.previous",
    label: "Previous chapter or bookmark",
    group: "queue",
    icon: SkipBack,
    isEnabled: hasMarkers,
    run: () => stepTo(-1),
  },
  {
    id: "bookmarks.rename_selected",
    label: "Rename",
    group: "context",
    icon: Pencil,
    isEnabled: () => bookmarks().selectedId !== null,
    run: () => {
      const id = bookmarks().selectedId;
      if (id !== null) startRename(id);
      return undefined;
    },
  },
  {
    id: "bookmarks.remove_selected",
    label: "Delete",
    group: "context",
    icon: Trash2,
    isEnabled: () => bookmarks().selectedId !== null,
    run: async () => {
      const id = bookmarks().selectedId;
      if (id !== null) await removeBookmark(id);
      return undefined;
    },
  },
  {
    id: "presets.save_current",
    label: "Save current settings as preset…",
    group: "app",
    icon: Save,
    isEnabled: hasMedia,
    run: () => {
      // The form lives in the preset strip at the bottom of the sidebar.
      const ui = useUiStore.getState();
      if (ui.dock !== "sidebar") ui.setDock("sidebar");
      usePresetStore.getState().requestSave();
      return undefined;
    },
  },
  {
    id: "presets.update_current",
    label: "Update applied preset",
    group: "app",
    icon: Save,
    isEnabled: () => (player()?.appliedPresets.length ?? 0) > 0 && Boolean(player()?.presetDirty),
    run: async () => {
      const id = await presetUpdateCurrent();
      const name = usePresetStore.getState().presets.find((p) => p.id === id)?.name ?? id;
      useOsdStore.getState().pushToast({ kind: "success", text: `Preset "${name}" updated` });
      return { icon: Save, label: "Preset updated", value: name };
    },
  },
  {
    id: "presets.apply",
    label: "Apply preset",
    group: "app",
    icon: Layers,
    isEnabled: hasMedia,
    run: async (ctx: RunContext) => {
      const id = String(ctx.args?.id ?? "");
      if (id === "") return undefined;
      await presetApply(id);
      const name = usePresetStore.getState().presets.find((p) => p.id === id)?.name ?? id;
      return { icon: Layers, label: "Preset", value: name };
    },
  },
];
