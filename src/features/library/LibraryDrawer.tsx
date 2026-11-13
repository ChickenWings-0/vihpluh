/**
 * The Library drawer (DESIGN_SYSTEM §11.1): slides in from the left between the bars, over
 * the video. Six sections share one virtualised list of rows; a row plays, opens, or both.
 * Playback is never interrupted by browsing, and the drawer stays open after enqueueing.
 */
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ArrowDown,
  ArrowUp,
  Check,
  ChevronLeft,
  ChevronRight,
  Folder,
  ListMusic,
  Pin,
  Play,
  Plus,
  Radio,
  RefreshCw,
  Search,
  X,
} from "lucide-react";
import {
  type ComponentType,
  type KeyboardEvent,
  type MouseEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { dispatch } from "@/actions";
import { playRow } from "@/actions/definitions/library";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { Button } from "@/components/Button";
import { Segmented } from "@/components/Controls";
import { IconButton } from "@/components/IconButton";
import { KeyCap } from "@/components/KeyCap";
import { DropdownMenu, MenuContent, type MenuEntry, separator } from "@/components/Menu";
import {
  type DuoProps,
  FolderDuo,
  LibraryDuo,
  MusicDuo,
  NetworkDuo,
  PlaylistDuo,
  RecentDuo,
  VideoDuo,
} from "@/icons/duotone";
import type { FolderKind } from "@/ipc/bindings/FolderKind";
import type { PlaylistSummary } from "@/ipc/bindings/PlaylistSummary";
import {
  appPickMedia,
  appReveal,
  libraryAddFolder,
  libraryClearHistory,
  libraryRemoveFolder,
  libraryRescan,
  playlistAddItems,
  playlistDelete,
  playlistExportM3u,
  playlistGet,
  playlistImportM3u,
  playlistList,
  playlistSave,
  streamsAddSource,
  streamsRefresh,
  streamsRemoveSource,
} from "@/ipc/commands";
import { onLibraryScan } from "@/ipc/events";
import { useShortcut } from "@/stores/keymap";
import { useLibraryStore } from "@/stores/library";
import { reportError, useOsdStore } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";
import { type Row, resolveSources, SECTIONS, type Section, VIEWS } from "./model";

const ROW_HEIGHT = 44;

const SECTION_META: Record<Section, { label: string; icon: ComponentType<DuoProps> }> = {
  video: { label: "Video", icon: VideoDuo },
  music: { label: "Music", icon: MusicDuo },
  streams: { label: "Streams", icon: NetworkDuo },
  recent: { label: "Recent", icon: RecentDuo },
  folders: { label: "Folders", icon: FolderDuo },
  playlists: { label: "Playlists", icon: PlaylistDuo },
};

const toast = (text: string, kind: "success" | "info" | "warn" = "success") =>
  useOsdStore.getState().pushToast({ kind, text, group: "library" });
const reload = () => useLibraryStore.getState().reload();
const ask = useUiStore.getState;
const store = useLibraryStore.getState;

/** Runs something that changes what the drawer lists, then lists it again. */
const then = (work: Promise<unknown>, done?: string) =>
  void work
    .then(() => {
      if (done) toast(done);
      reload();
    })
    .catch(reportError);

async function movePlaylistItem(id: string, index: number, by: number): Promise<void> {
  const playlist = await playlistGet(id);
  const to = index + by;
  const item = playlist.items[index];
  if (!item || to < 0 || to >= playlist.items.length) return;
  playlist.items.splice(index, 1);
  playlist.items.splice(to, 0, item);
  await playlistSave(playlist);
}

async function removePlaylistItem(id: string, index: number): Promise<void> {
  const playlist = await playlistGet(id);
  playlist.items.splice(index, 1);
  await playlistSave(playlist);
}

function addStreamSource(kind: "file" | "url"): void {
  if (kind === "file") {
    void appPickMedia("playlist")
      .then((picked) => {
        const path = picked[0];
        if (path) then(streamsAddSource("", path), "Stream list added");
      })
      .catch(reportError);
    return;
  }
  ask().ask({
    title: "Add stream list",
    label: "Address of an M3U list",
    placeholder: "https://…/list.m3u",
    confirm: "Add",
    onSubmit: async (text) => {
      await streamsAddSource("", text.trim());
      toast("Stream list added");
      reload();
    },
  });
}

function addLibraryFolder(kind: FolderKind): void {
  void appPickMedia("folder")
    .then((picked) => {
      const path = picked[0];
      if (path) then(libraryAddFolder(path, kind, false, true), "Folder added to the library");
    })
    .catch(reportError);
}

/** The menu for a row: playing first, then what only that kind of row can do. */
function rowMenu(row: Row, playlists: PlaylistSummary[]): MenuEntry[] {
  const item = (label: string, onSelect: () => void, disabled = false): MenuEntry => ({
    kind: "item",
    label,
    onSelect,
    disabled,
  });
  const play = (mode: "replace" | "play_next" | "enqueue") => () =>
    void playRow(row, mode).catch(reportError);
  const entries: MenuEntry[] = [
    item("Play now", play("replace")),
    item("Play next", play("play_next")),
    item("Add to queue", play("enqueue")),
  ];
  if (row.kind !== "playlist" && row.kind !== "source") {
    const add = (id: string, name: string) => () =>
      void resolveSources(row)
        .then((sources) => playlistAddItems(id, sources))
        .then(() => toast(`Added to "${name}"`))
        .catch(reportError);
    entries.push({
      kind: "submenu",
      label: "Add to playlist",
      entries: [
        ...playlists.map((p) => item(p.name, add(p.id, p.name))),
        ...(playlists.length > 0 ? [separator] : []),
        item("New playlist…", () =>
          ask().ask({
            title: "New playlist",
            label: "Playlist name",
            confirm: "Create",
            onSubmit: async (name) => {
              const sources = await resolveSources(row);
              const id = await playlistSave({
                schema: 1,
                id: "",
                name: name.trim(),
                createdAt: "",
                updatedAt: "",
                items: [],
              });
              await playlistAddItems(id, sources);
              toast(`Added to "${name.trim()}"`);
            },
          }),
        ),
      ],
    });
  }
  const path = row.path;
  if (path) {
    entries.push(
      separator,
      item("Show in folder", () => void appReveal(path).catch(reportError)),
    );
  }
  const { folderId, playlistId, sourceId, index } = row;
  if (row.kind === "dir" && path) {
    entries.push(
      item("Pin this folder", () => then(libraryAddFolder(path, "mixed", true, false), "Pinned")),
    );
  }
  if (row.kind === "root" && folderId !== undefined && path) {
    entries.push(
      row.pinned
        ? item("Unpin", () =>
            then(
              row.indexed
                ? libraryAddFolder(path, "mixed", false, true)
                : libraryRemoveFolder(folderId),
            ),
          )
        : item("Pin", () => then(libraryAddFolder(path, "mixed", true, false))),
    );
    if (row.indexed) {
      entries.push(
        item("Scan again", () => void libraryRescan(folderId).catch(reportError)),
        item("Remove from library", () => then(libraryRemoveFolder(folderId), "Folder removed")),
      );
    }
  }
  if (row.kind === "playlist" && playlistId) {
    entries.push(
      separator,
      item("Rename…", () =>
        ask().ask({
          title: "Rename playlist",
          label: "Playlist name",
          initial: row.title,
          confirm: "Rename",
          onSubmit: async (name) => {
            const playlist = await playlistGet(playlistId);
            await playlistSave({ ...playlist, name: name.trim() });
            reload();
          },
        }),
      ),
      item("Export as M3U8…", () =>
        ask().ask({
          title: "Export playlist",
          label: "File to write",
          placeholder: "D:/Playlists/name.m3u8",
          confirm: "Export",
          onSubmit: async (file) => {
            await playlistExportM3u(playlistId, file.trim());
            toast("Playlist exported");
          },
        }),
      ),
      item("Delete playlist", () => then(playlistDelete(playlistId), "Playlist deleted")),
    );
  }
  if (row.kind === "playlist_item" && playlistId && index !== undefined) {
    entries.push(
      separator,
      item("Move up", () => then(movePlaylistItem(playlistId, index, -1)), index === 0),
      item("Move down", () => then(movePlaylistItem(playlistId, index, 1))),
      item("Remove from playlist", () => then(removePlaylistItem(playlistId, index))),
    );
  }
  if (row.kind === "source" && sourceId !== undefined) {
    entries.push(
      separator,
      item("Refresh", () => then(streamsRefresh(sourceId), "Stream list refreshed")),
      item("Remove", () => then(streamsRemoveSource(sourceId), "Stream list removed")),
    );
  }
  return entries;
}

const ROW_ICON: Partial<Record<Row["kind"], typeof Folder>> = {
  dir: Folder,
  root: Folder,
  playlist: ListMusic,
  source: Radio,
  channel: Radio,
};

function RowView({
  row,
  start,
  selected,
  onMenu,
}: {
  row: Row;
  start: number;
  selected: boolean;
  onMenu: (e: MouseEvent, row: Row) => void;
}) {
  const select = useLibraryStore((s) => s.select);
  const open = useLibraryStore((s) => s.open);
  const Icon = ROW_ICON[row.kind];
  const activate = () => {
    if (row.open && !row.sources) open(row.open);
    else void playRow(row, "replace").catch(reportError);
  };
  const stop = (e: MouseEvent, run: () => void) => {
    e.stopPropagation();
    run();
  };
  const small =
    "flex h-6 w-6 items-center justify-center rounded-control text-fg-2 hover:bg-active hover:text-fg";
  return (
    // Keyboard use lives on the listbox (arrows, Enter…), not on each row.
    // biome-ignore lint/a11y/useKeyWithClickEvents: rows are options of a keyboard-driven listbox
    <div
      role="option"
      aria-selected={selected}
      tabIndex={-1}
      data-row={row.key}
      title={row.path ?? row.sources?.[0]}
      onClick={() => {
        select(row.key);
        // Things that only open do so on a single click, like folders anywhere.
        if (row.open && !row.sources) open(row.open);
      }}
      onDoubleClick={activate}
      onContextMenu={(e) => onMenu(e, row)}
      style={{ transform: `translateY(${start}px)` }}
      className={`group absolute inset-x-0 top-0 flex h-11 cursor-default items-center gap-2 border-l-2 pr-1 pl-2 ${
        selected ? "border-accent bg-accent-soft" : "border-transparent hover:bg-hover"
      }`}
    >
      {Icon && <Icon size={16} strokeWidth={1.5} className="shrink-0 text-fg-2" aria-hidden />}
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="flex items-center gap-1.5">
          <span className="truncate text-base text-fg">{row.title}</span>
          {row.finished && (
            <Check size={12} strokeWidth={2} className="shrink-0 text-fg-3" aria-label="Watched" />
          )}
        </span>
        {row.meta && <span className="truncate text-fg-3 text-sm">{row.meta}</span>}
      </span>
      {row.right && (
        <span className="tabular shrink-0 text-fg-2 text-sm group-hover:hidden">{row.right}</span>
      )}
      <span className="hidden shrink-0 group-hover:flex">
        {row.kind === "playlist_item" && row.playlistId && row.index !== undefined && (
          <>
            <button
              type="button"
              tabIndex={-1}
              aria-label="Move up"
              className={small}
              onClick={(e) =>
                stop(e, () => then(movePlaylistItem(row.playlistId ?? "", row.index ?? 0, -1)))
              }
            >
              <ArrowUp size={13} strokeWidth={1.5} aria-hidden />
            </button>
            <button
              type="button"
              tabIndex={-1}
              aria-label="Move down"
              className={small}
              onClick={(e) =>
                stop(e, () => then(movePlaylistItem(row.playlistId ?? "", row.index ?? 0, 1)))
              }
            >
              <ArrowDown size={13} strokeWidth={1.5} aria-hidden />
            </button>
          </>
        )}
        <button
          type="button"
          tabIndex={-1}
          aria-label={`Play ${row.title}`}
          className={small}
          onClick={(e) => stop(e, () => void playRow(row, "replace").catch(reportError))}
        >
          <Play size={13} strokeWidth={1.5} aria-hidden />
        </button>
        <button
          type="button"
          tabIndex={-1}
          aria-label={`Add ${row.title} to the queue`}
          className={small}
          onClick={(e) => stop(e, () => void playRow(row, "enqueue").catch(reportError))}
        >
          <Plus size={14} strokeWidth={1.5} aria-hidden />
        </button>
      </span>
      {row.open && !row.sources && (
        <ChevronRight size={14} strokeWidth={1.5} className="shrink-0 text-fg-3" aria-hidden />
      )}
      {row.progress !== undefined && (
        <span className="absolute inset-x-2 bottom-0.5 h-0.5 bg-track" aria-hidden>
          <span className="block h-full bg-accent" style={{ width: `${row.progress * 100}%` }} />
        </span>
      )}
    </div>
  );
}

/** Buttons that belong to the place the drawer is in. */
function PlaceActions() {
  const section = useLibraryStore((s) => s.section);
  const trail = useLibraryStore((s) => s.trail);
  const here = trail[trail.length - 1];
  const [menu, setMenu] = useState<MenuEntry[] | null>(null);
  const item = (label: string, onSelect: () => void): MenuEntry => ({
    kind: "item",
    label,
    onSelect,
  });
  const folderKinds: MenuEntry[] = [
    item("Videos", () => addLibraryFolder("video")),
    item("Music", () => addLibraryFolder("music")),
    item("Videos and music", () => addLibraryFolder("mixed")),
  ];
  const menuButton = (label: string, entries: MenuEntry[]) => (
    <DropdownMenu.Root
      open={menu === entries}
      modal={false}
      onOpenChange={(open) => setMenu(open ? entries : null)}
    >
      <DropdownMenu.Trigger asChild>
        <Button size="sm" variant="ghost" icon={Plus} keepFocus>
          {label}
        </Button>
      </DropdownMenu.Trigger>
      <MenuContent entries={entries} align="start" />
    </DropdownMenu.Root>
  );
  if (here?.type === "dir") {
    return (
      <Button
        size="sm"
        variant="ghost"
        icon={Pin}
        onClick={() => then(libraryAddFolder(here.path, "mixed", true, false), "Pinned")}
      >
        Pin this folder
      </Button>
    );
  }
  if (here) return null;
  switch (section) {
    case "video":
    case "music":
    case "folders":
      return menuButton("Add folder…", folderKinds);
    case "streams":
      return (
        <>
          {menuButton("Add M3U…", [
            item("From a file…", () => addStreamSource("file")),
            item("From an address…", () => addStreamSource("url")),
          ])}
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void dispatch("app.open_url", { trigger: "button" })}
          >
            Open URL…
          </Button>
        </>
      );
    case "recent":
      return (
        <Button
          size="sm"
          variant="ghost"
          onClick={() => then(libraryClearHistory(), "History cleared")}
        >
          Clear history
        </Button>
      );
    case "playlists":
      return (
        <Button
          size="sm"
          variant="ghost"
          icon={Plus}
          onClick={() =>
            void appPickMedia("playlist")
              .then((picked) => {
                const path = picked[0];
                if (path) then(playlistImportM3u(path), "Playlist imported");
              })
              .catch(reportError)
          }
        >
          Import M3U…
        </Button>
      );
  }
}

const EMPTY: Record<Section, string> = {
  video: "No videos yet. Add a folder to the library.",
  music: "No music yet. Add a folder to the library.",
  streams: "No stream lists. Add an M3U, or open an address.",
  recent: "Nothing played yet.",
  folders: "No folders yet. Add one to the library, or pin one while browsing.",
  playlists: "No playlists. Save the queue as one from the queue's menu.",
};

function Rows({ onMenu }: { onMenu: (e: MouseEvent, row: Row) => void }) {
  const rows = useLibraryStore((s) => s.rows);
  const total = useLibraryStore((s) => s.total);
  const loading = useLibraryStore((s) => s.loading);
  const selectedKey = useLibraryStore((s) => s.selectedKey);
  const section = useLibraryStore((s) => s.section);
  const search = useLibraryStore((s) => s.search);
  const inside = useLibraryStore((s) => s.trail.length > 0);
  const select = useLibraryStore((s) => s.select);
  const loadMore = useLibraryStore((s) => s.loadMore);
  const scroller = useRef<HTMLDivElement>(null);
  const virtual = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  });
  const items = virtual.getVirtualItems();
  const lastShown = items[items.length - 1]?.index ?? 0;
  // The next page is fetched as the end comes into view.
  useEffect(() => {
    if (rows.length < total && lastShown >= rows.length - 20) loadMore();
  }, [lastShown, rows.length, total, loadMore]);

  // Keyboard browsing: a freshly listed place has its first row selected, so Enter and the
  // arrows work at once after opening something.
  useEffect(() => {
    const first = rows[0];
    if (selectedKey === null && first && document.activeElement === scroller.current) {
      select(first.key);
    }
  }, [rows, selectedKey, select]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const index = rows.findIndex((r) => r.key === selectedKey);
    const move = (to: number) => {
      const row = rows[Math.max(0, Math.min(rows.length - 1, to))];
      if (!row) return;
      select(row.key);
      virtual.scrollToIndex(rows.indexOf(row));
    };
    const run = (id: string) => void dispatch(id, { trigger: "key" });
    const row = rows[index];
    // Alt+arrows belong to other shortcuts (subtitle position).
    if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) return;
    if (e.key === "ArrowDown") move(index + 1);
    else if (e.key === "ArrowUp") move(index - 1);
    else if (e.key === "Home") move(0);
    else if (e.key === "End") move(rows.length - 1);
    else if (e.key === "PageDown") move(index + 10);
    else if (e.key === "PageUp") move(index - 10);
    else if (e.key === "Enter" && e.shiftKey) run("library.play_next_selected");
    else if (e.key === "Enter" && e.ctrlKey) run("library.enqueue_selected");
    else if (e.key === "Enter") run("library.play_selected");
    else if (e.key === "Backspace") run("library.go_up");
    else if (e.key === "ArrowRight" && row?.open) useLibraryStore.getState().open(row.open);
    else if (e.key === "ArrowLeft") run("library.go_up");
    else return;
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <div
      ref={scroller}
      role="listbox"
      tabIndex={0}
      aria-label="Library"
      data-library-list
      onKeyDown={onKeyDown}
      onFocus={() => {
        if (selectedKey === null && rows[0]) select(rows[0].key);
      }}
      className="min-h-0 flex-1 overflow-y-auto overscroll-contain outline-none"
    >
      <div className="relative w-full" style={{ height: virtual.getTotalSize() }}>
        {items.map((item) => {
          const row = rows[item.index];
          if (!row) return null;
          return (
            <RowView
              key={row.key}
              row={row}
              start={item.start}
              selected={row.key === selectedKey}
              onMenu={onMenu}
            />
          );
        })}
      </div>
      {rows.length === 0 && !loading && (
        <div className="flex flex-col items-center gap-2 px-6 py-10 text-center">
          <LibraryDuo size={32} />
          <p className="text-base text-fg-3">
            {search.trim() ? "Nothing matches." : inside ? "Nothing here." : EMPTY[section]}
          </p>
        </div>
      )}
    </div>
  );
}

/** Closes the drawer on a click outside it (rendered under the dock, over the video). */
export function LibraryScrim() {
  const open = useUiStore((s) => s.library);
  const zen = useUiStore((s) => s.zen);
  const setLibrary = useUiStore((s) => s.setLibrary);
  if (!open || zen) return null;
  return (
    <div
      aria-hidden="true"
      className="absolute inset-x-0 top-10 bottom-13 z-(--z-dock)"
      onPointerDown={(e) => {
        e.preventDefault();
        setLibrary(false);
        focusPlayerRoot();
      }}
    />
  );
}

export function LibraryDrawer() {
  const open = useUiStore((s) => s.library);
  const zen = useUiStore((s) => s.zen);
  const setLibrary = useUiStore((s) => s.setLibrary);
  const setKeyContext = useUiStore((s) => s.setKeyContext);
  const hold = useUiStore((s) => s.hold);
  const section = useLibraryStore((s) => s.section);
  const view = useLibraryStore((s) => s.views[s.section]);
  const trail = useLibraryStore((s) => s.trail);
  const search = useLibraryStore((s) => s.search);
  const total = useLibraryStore((s) => s.total);
  const scan = useLibraryStore((s) => s.scan);
  const focusSearch = useLibraryStore((s) => s.focusSearch);
  const searchKey = useShortcut("library.focus_search");
  const input = useRef<HTMLInputElement>(null);
  const [draft, setDraft] = useState(search);
  const [menu, setMenu] = useState<{ x: number; y: number; row: Row } | null>(null);
  const [playlists, setPlaylists] = useState<PlaylistSummary[]>([]);
  const shown = open && !zen;

  // The drawer lists its place when it opens, and holds the bars while it is open.
  useEffect(() => {
    hold("library", shown);
    if (shown) store().reload();
    return () => hold("library", false);
  }, [shown, hold]);

  // Scan progress; a finished scan refreshes what is listed.
  useEffect(
    () =>
      onLibraryScan((progress) => {
        store().setScan(progress.done ? null : progress);
        if (progress.done && useUiStore.getState().library) store().reload();
      }),
    [],
  );

  // Typing searches after a short pause.
  useEffect(() => {
    const timer = setTimeout(() => store().setSearch(draft), 150);
    return () => clearTimeout(timer);
  }, [draft]);
  useEffect(() => setDraft(search), [search]);
  useEffect(() => {
    if (focusSearch > 0) input.current?.focus();
  }, [focusSearch]);

  const views = trail.length === 0 && !search.trim() ? VIEWS[section] : undefined;
  const menuEntries = useMemo(() => (menu ? rowMenu(menu.row, playlists) : []), [menu, playlists]);
  if (!shown) return null;

  const onMenu = (e: MouseEvent, row: Row) => {
    e.preventDefault();
    store().select(row.key);
    void playlistList()
      .then(setPlaylists)
      .catch(() => undefined);
    setMenu({ x: e.clientX, y: e.clientY, row });
  };

  return (
    <aside
      aria-label="Library"
      data-library
      onFocus={() => setKeyContext("library")}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setKeyContext("global");
      }}
      className="drawer-in absolute top-10 bottom-13 left-0 z-(--z-dock) flex w-95 max-w-[calc(100%-48px)] flex-col border-line border-r bg-surface/92 shadow-float"
    >
      <div className="flex shrink-0 items-center gap-1 p-2 pb-1">
        <label className="flex h-8 min-w-0 flex-1 items-center gap-2 rounded-control border border-line bg-raised px-2 focus-within:border-accent">
          <Search size={14} strokeWidth={1.5} className="shrink-0 text-fg-3" aria-hidden />
          <input
            ref={input}
            type="text"
            value={draft}
            placeholder={`Search ${SECTION_META[section].label.toLowerCase()}…`}
            aria-label="Search the library"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown" || e.key === "Enter") {
                e.preventDefault();
                e.stopPropagation();
                document.querySelector<HTMLElement>("[data-library-list]")?.focus();
              } else if (e.key === "Escape" && draft !== "") {
                e.preventDefault();
                e.stopPropagation();
                setDraft("");
              }
            }}
            className="min-w-0 flex-1 bg-transparent text-base text-fg outline-none placeholder:text-fg-3"
          />
          {searchKey && draft === "" && <KeyCap binding={searchKey} />}
        </label>
        <IconButton
          icon={X}
          label="Close library"
          size="panel"
          tooltipSide="bottom"
          onClick={() => setLibrary(false)}
        />
      </div>

      <div role="tablist" aria-label="Library sections" className="flex shrink-0 px-1">
        {SECTIONS.map((id) => {
          const { label, icon: Icon } = SECTION_META[id];
          const active = id === section;
          return (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={active}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => store().setSection(id)}
              className={`flex min-w-0 flex-1 flex-col items-center gap-0.5 rounded-control border-b-2 px-1 pt-1.5 pb-1 text-xs ${
                active ? "border-accent text-fg" : "border-transparent text-fg-2 hover:bg-hover"
              }`}
            >
              <Icon size={20} />
              <span className="w-full truncate text-center">{label}</span>
            </button>
          );
        })}
      </div>

      <div className="flex min-h-9 shrink-0 flex-wrap items-center gap-1 border-line border-b px-2 py-1">
        {trail.length > 0 ? (
          <nav aria-label="Where you are" className="flex min-w-0 flex-1 items-center gap-0.5">
            <IconButton
              icon={ChevronLeft}
              label="Up one level"
              size="panel"
              actionId="library.go_up"
            />
            <button
              type="button"
              onClick={() => store().up(0)}
              className="shrink-0 rounded-control px-1 text-fg-2 text-sm hover:bg-hover hover:text-fg"
            >
              {SECTION_META[section].label}
            </button>
            {trail.map((crumb, depth) => (
              <span key={JSON.stringify(crumb)} className="flex min-w-0 items-center gap-0.5">
                <ChevronRight size={12} className="shrink-0 text-fg-3" aria-hidden />
                <button
                  type="button"
                  disabled={depth === trail.length - 1}
                  onClick={() => store().up(depth + 1)}
                  className="truncate rounded-control px-1 text-sm enabled:text-fg-2 enabled:hover:bg-hover enabled:hover:text-fg disabled:text-fg"
                >
                  {crumb.label}
                </button>
              </span>
            ))}
          </nav>
        ) : views ? (
          <Segmented
            label="View"
            className="min-w-40 flex-1"
            options={views}
            value={view ?? null}
            onChange={(v) => store().setView(v)}
          />
        ) : (
          <span className="flex-1 text-fg-3 text-sm">
            {search.trim() ? `${total} result${total === 1 ? "" : "s"}` : ""}
          </span>
        )}
        <PlaceActions />
        {section === "folders" && trail.length === 0 && (
          <IconButton
            icon={RefreshCw}
            label="Scan the library again"
            size="panel"
            onClick={() => void libraryRescan().catch(reportError)}
          />
        )}
      </div>

      <Rows onMenu={onMenu} />

      {scan && (
        <div
          role="status"
          className="tabular shrink-0 border-line border-t px-3 py-1.5 text-fg-2 text-sm"
        >
          Scanning… {scan.scanned.toLocaleString()}
          {scan.total !== null ? ` of ${scan.total.toLocaleString()}` : ""} files
        </div>
      )}

      {menu && (
        <DropdownMenu.Root
          open
          modal={false}
          onOpenChange={(next) => {
            if (!next) setMenu(null);
          }}
        >
          <DropdownMenu.Trigger asChild>
            <div
              aria-hidden="true"
              className="pointer-events-none fixed h-0 w-0"
              style={{ left: menu.x, top: menu.y }}
            />
          </DropdownMenu.Trigger>
          <MenuContent entries={menuEntries} align="start" side="bottom" />
        </DropdownMenu.Root>
      )}
    </aside>
  );
}
