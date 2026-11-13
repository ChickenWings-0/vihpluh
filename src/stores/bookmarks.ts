/**
 * The bookmarks of the file that is playing, plus the Bookmarks tab's view state (selection,
 * the row being renamed, the filter). The list is the core's: fetched for each new media key
 * and again whenever `bookmarks://changed` names it.
 */
import { create } from "zustand";
import type { Bookmark } from "@/ipc/bindings/Bookmark";
import { bookmarkList } from "@/ipc/commands";
import { onBookmarksChanged } from "@/ipc/events";
import type { MarkerFilter } from "@/lib/markers";
import { usePlayerStore } from "./player";

interface BookmarkStore {
  mediaKey: string;
  bookmarks: Bookmark[];
  selectedId: number | null;
  renamingId: number | null;
  filter: MarkerFilter;
  setBookmarks: (mediaKey: string, bookmarks: Bookmark[]) => void;
  select: (id: number | null) => void;
  rename: (id: number | null) => void;
  setFilter: (filter: MarkerFilter) => void;
}

const NONE: Bookmark[] = [];

export const useBookmarkStore = create<BookmarkStore>((set, get) => ({
  mediaKey: "",
  bookmarks: NONE,
  selectedId: null,
  renamingId: null,
  filter: "all",
  setBookmarks: (mediaKey, bookmarks) => {
    const ids = new Set(bookmarks.map((b) => b.id));
    const keep = (id: number | null) => (id !== null && ids.has(id) ? id : null);
    set({
      mediaKey,
      bookmarks,
      selectedId: keep(get().selectedId),
      renamingId: keep(get().renamingId),
    });
  },
  select: (selectedId) => set({ selectedId }),
  rename: (renamingId) => set({ renamingId, selectedId: renamingId ?? get().selectedId }),
  setFilter: (filter) => set({ filter }),
}));

/** Keeps the list in step with the playing media and with changes made anywhere. */
export function startBookmarkSync(): () => void {
  let wanted = "";
  const load = (key: string) => {
    wanted = key;
    if (key === "") {
      useBookmarkStore.getState().setBookmarks("", NONE);
      return;
    }
    bookmarkList(key)
      .then((list) => {
        if (wanted === key) useBookmarkStore.getState().setBookmarks(key, list);
      })
      .catch(() => undefined);
  };
  const unsubscribe = usePlayerStore.subscribe((store) => {
    const key = store.state?.media?.key ?? "";
    if (key !== wanted) load(key);
  });
  const unlisten = onBookmarksChanged((event) => {
    if (event.mediaKey === wanted) load(wanted);
  });
  return () => {
    unsubscribe();
    unlisten();
  };
}
