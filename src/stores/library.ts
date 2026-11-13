/**
 * Where the Library drawer is (section, view, the trail of opened rows, the search text),
 * what that place lists, and which row is selected. View state only: the rows themselves come
 * from the core each time the place changes.
 */
import { create } from "zustand";
import {
  type Crumb,
  DEFAULT_VIEW,
  loadRows,
  PAGE,
  type Row,
  type Section,
  type ViewId,
} from "@/features/library/model";
import type { ScanProgress } from "@/ipc/bindings/ScanProgress";
import { reportError } from "./osd";

interface LibraryStore {
  section: Section;
  views: Partial<Record<Section, ViewId>>;
  trail: Crumb[];
  search: string;
  rows: Row[];
  total: number;
  loading: boolean;
  selectedKey: string | null;
  scan: ScanProgress | null;
  /** Bumped to move keyboard focus to the search field. */
  focusSearch: number;

  setSection: (section: Section) => void;
  setView: (view: ViewId) => void;
  open: (crumb: Crumb) => void;
  /** Back one level (or to `depth` crumbs). Returns false at the top. */
  up: (depth?: number) => boolean;
  setSearch: (search: string) => void;
  select: (key: string | null) => void;
  reload: () => void;
  loadMore: () => void;
  setScan: (scan: ScanProgress | null) => void;
  requestSearchFocus: () => void;
}

/** Each load gets a number; an answer to an older question is dropped. */
let generation = 0;

export const useLibraryStore = create<LibraryStore>((set, get) => {
  const load = (offset: number) => {
    const { section, views, trail, search } = get();
    const mine = ++generation;
    set({ loading: true });
    loadRows(section, views[section] ?? DEFAULT_VIEW[section], trail, search, offset)
      .then((listing) => {
        if (mine !== generation) return;
        const rows = offset === 0 ? listing.rows : [...get().rows, ...listing.rows];
        const keys = new Set(rows.map((r) => r.key));
        const selected = get().selectedKey;
        set({
          rows,
          total: listing.total,
          loading: false,
          selectedKey: selected && keys.has(selected) ? selected : null,
        });
      })
      .catch((error) => {
        if (mine !== generation) return;
        set({ loading: false, rows: offset === 0 ? [] : get().rows });
        reportError(error);
      });
  };
  const go = (patch: Partial<LibraryStore>) => {
    set({ ...patch, rows: [], total: 0, selectedKey: null });
    load(0);
  };

  return {
    section: "video",
    views: { ...DEFAULT_VIEW },
    trail: [],
    search: "",
    rows: [],
    total: 0,
    loading: false,
    selectedKey: null,
    scan: null,
    focusSearch: 0,

    setSection: (section) => go({ section, trail: [], search: "" }),
    setView: (view) => go({ views: { ...get().views, [get().section]: view }, trail: [] }),
    open: (crumb) => go({ trail: [...get().trail, crumb], search: "" }),
    up: (depth) => {
      const trail = get().trail;
      if (trail.length === 0) return false;
      go({ trail: trail.slice(0, depth ?? trail.length - 1) });
      return true;
    },
    setSearch: (search) => {
      if (search === get().search) return;
      // Searching looks across the whole section, not inside an opened row.
      go({ search, trail: search.trim() ? [] : get().trail });
    },
    select: (selectedKey) => set({ selectedKey }),
    reload: () => load(0),
    loadMore: () => {
      const { rows, total, loading } = get();
      if (!loading && rows.length < total && rows.length % PAGE === 0) load(rows.length);
    },
    setScan: (scan) => set({ scan }),
    requestSearchFocus: () => set({ focusSearch: get().focusSearch + 1 }),
  };
});

export const selectedRow = (): Row | null => {
  const { rows, selectedKey } = useLibraryStore.getState();
  return rows.find((r) => r.key === selectedKey) ?? null;
};
