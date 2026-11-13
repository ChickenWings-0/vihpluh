/**
 * The Library drawer's data (DESIGN_SYSTEM §11.1): every section lists the same kind of row,
 * so one list component, one set of keys, and one menu serve all of them. A row either plays
 * (`sources`), opens (`open`), or both (a group plays everything inside it).
 */
import type { GroupEntry } from "@/ipc/bindings/GroupEntry";
import type { LibraryMedia } from "@/ipc/bindings/LibraryMedia";
import type { LibraryView } from "@/ipc/bindings/LibraryView";
import type { MediaKind } from "@/ipc/bindings/MediaKind";
import type { RecentEntry } from "@/ipc/bindings/RecentEntry";
import {
  libraryBrowse,
  libraryFolders,
  libraryQuery,
  libraryRecent,
  playlistGet,
  playlistList,
  streamsGroups,
  streamsList,
  streamsSources,
} from "@/ipc/commands";
import { formatTime } from "@/lib/time";

export type Section = "video" | "music" | "streams" | "recent" | "folders" | "playlists";
export const SECTIONS: Section[] = ["video", "music", "streams", "recent", "folders", "playlists"];

export type ViewId = "all" | "series" | "folders" | "albums" | "artists" | "tracks";
export const VIEWS: Partial<Record<Section, { value: ViewId; label: string }[]>> = {
  video: [
    { value: "all", label: "All" },
    { value: "series", label: "Series" },
    { value: "folders", label: "Folders" },
  ],
  music: [
    { value: "albums", label: "Albums" },
    { value: "artists", label: "Artists" },
    { value: "tracks", label: "Tracks" },
  ],
};
export const DEFAULT_VIEW: Partial<Record<Section, ViewId>> = { video: "series", music: "albums" };

/** One step into a section: what a row opens. */
export type Crumb =
  | { type: "group"; kind: MediaKind; view: LibraryView; key: string; label: string }
  | { type: "dir"; path: string; label: string }
  | { type: "playlist"; id: string; label: string }
  | { type: "source"; id: number; label: string }
  | { type: "channels"; sourceId: number; group: string; label: string };

export type RowKind =
  | "media"
  | "group"
  | "dir"
  | "root"
  | "playlist"
  | "playlist_item"
  | "source"
  | "channel";

export interface Row {
  key: string;
  kind: RowKind;
  title: string;
  /** Second line: series and episode, artist, folder, group. */
  meta?: string;
  /** Right-aligned: a duration or a count. */
  right?: string;
  /** What "play" plays, when it is known without asking. */
  sources?: string[];
  /** Opening the row goes here. */
  open?: Crumb;
  /** A local file or folder, for "Show in folder" and pinning. */
  path?: string;
  /** 0–1 of the file played (the 2 px progress line). */
  progress?: number;
  finished?: boolean;
  /** Ids for row-specific menu items. */
  folderId?: number;
  pinned?: boolean;
  indexed?: boolean;
  playlistId?: string;
  sourceId?: number;
  /** Position within an opened playlist. */
  index?: number;
}

export interface Listing {
  rows: Row[];
  total: number;
}

export const PAGE = 200;

const baseName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

/** `2 h 44 m`, `47 m`, `3:05` — long things in hours and minutes, short ones exactly. */
export function formatLength(seconds: number | null | undefined): string | undefined {
  if (seconds === null || seconds === undefined || seconds <= 0) return undefined;
  if (seconds < 600) return formatTime(seconds);
  const minutes = Math.round(seconds / 60);
  return minutes >= 60 ? `${Math.floor(minutes / 60)} h ${minutes % 60} m` : `${minutes} m`;
}

export function episodeLabel(season: number | null, episode: number | null): string | undefined {
  if (episode === null) return undefined;
  const e = String(episode).padStart(2, "0");
  return season === null ? `Episode ${e}` : `S${String(season).padStart(2, "0")}E${e}`;
}

const progressOf = (position: number | null, duration: number | null) =>
  position && duration && duration > 0 ? Math.min(1, position / duration) : undefined;

export function mediaRow(media: LibraryMedia, inGroup: boolean): Row {
  const parts =
    media.kind === "audio"
      ? [
          inGroup && media.trackNo ? `Track ${media.trackNo}` : null,
          media.artist,
          inGroup ? null : media.album,
        ]
      : [inGroup ? null : media.seriesKey, episodeLabel(media.season, media.episode)];
  return {
    key: `m:${media.path}`,
    kind: "media",
    title: media.title,
    meta: parts.filter(Boolean).join(" · ") || undefined,
    right: formatLength(media.duration),
    sources: [media.path],
    path: media.path,
    progress: media.finished ? undefined : progressOf(media.position, media.duration),
    finished: media.finished,
  };
}

export function groupRow(group: GroupEntry, kind: MediaKind, view: LibraryView): Row {
  const unknown =
    view === "albums" ? "Unknown album" : view === "artists" ? "Unknown artist" : "Unknown";
  const title = group.title.trim() === "" ? unknown : group.title;
  const noun = kind === "audio" ? "track" : "episode";
  return {
    key: `g:${view}:${group.key}`,
    kind: "group",
    title,
    meta: [group.subtitle, group.year].filter(Boolean).join(" · ") || undefined,
    right: `${group.count} ${noun}${group.count === 1 ? "" : "s"}`,
    open: { type: "group", kind, view, key: group.key, label: title },
  };
}

export function recentRow(entry: RecentEntry): Row {
  return {
    key: `r:${entry.mediaKey}`,
    kind: "media",
    title: entry.title,
    meta: /^[a-z][a-z0-9+.-]*:\/\//i.test(entry.source)
      ? entry.source
      : baseName(entry.source.replace(/[\\/][^\\/]*$/, "")),
    right: formatLength(entry.duration),
    sources: [entry.source],
    path: /^[a-z][a-z0-9+.-]*:\/\//i.test(entry.source) ? undefined : entry.source,
    progress: entry.finished ? undefined : progressOf(entry.position, entry.duration),
    finished: entry.finished,
  };
}

async function query(
  kind: MediaKind,
  view: LibraryView,
  offset: number,
  parent?: string,
  search?: string,
): Promise<Listing> {
  const page = await libraryQuery({ kind, view, parent, search, limit: PAGE, offset });
  const inGroup = parent !== undefined;
  return {
    total: page.total,
    rows: page.items.map((item) =>
      item.type === "group" ? groupRow(item, kind, view) : mediaRow(item, inGroup),
    ),
  };
}

async function browse(path: string): Promise<Listing> {
  const listing = await libraryBrowse(path);
  const dirs: Row[] = listing.dirs.map((dir) => ({
    key: `d:${dir.path}`,
    kind: "dir",
    title: dir.name,
    path: dir.path,
    open: { type: "dir", path: dir.path, label: dir.name },
  }));
  const files = listing.files.map((file) => mediaRow(file, true));
  return { rows: [...dirs, ...files], total: dirs.length + files.length };
}

async function roots(kinds: string[] | null): Promise<Listing> {
  const folders = await libraryFolders();
  const rows: Row[] = folders
    .filter((f) => (kinds === null ? true : f.indexed && kinds.includes(f.kind)))
    .map((f) => ({
      key: `f:${f.id}`,
      kind: "root",
      title: baseName(f.path),
      meta: f.path,
      right: f.indexed ? `${f.count} file${f.count === 1 ? "" : "s"}` : "pinned",
      path: f.path,
      folderId: f.id,
      pinned: f.pinned,
      indexed: f.indexed,
      open: { type: "dir", path: f.path, label: baseName(f.path) },
    }));
  return { rows, total: rows.length };
}

/** The rows of a place in the drawer. `offset` pages the long, indexed listings. */
export async function loadRows(
  section: Section,
  view: ViewId | undefined,
  trail: Crumb[],
  search: string,
  offset: number,
): Promise<Listing> {
  const here = trail[trail.length - 1];
  const text = search.trim();
  if (here?.type === "dir") return offset === 0 ? browse(here.path) : { rows: [], total: 0 };
  if (here?.type === "group") return query(here.kind, here.view, offset, here.key);
  if (here?.type === "playlist") {
    if (offset > 0) return { rows: [], total: 0 };
    const playlist = await playlistGet(here.id);
    const rows: Row[] = playlist.items.map((item, index) => ({
      key: `pi:${index}:${item.source}`,
      kind: "playlist_item",
      title: item.title,
      meta: baseName(item.source.replace(/[\\/][^\\/]*$/, "")),
      right: formatLength(item.duration),
      sources: [item.source],
      path: /^[a-z][a-z0-9+.-]*:\/\//i.test(item.source) ? undefined : item.source,
      playlistId: playlist.id,
      index,
    }));
    return { rows, total: rows.length };
  }
  if (here?.type === "source") {
    if (offset > 0) return { rows: [], total: 0 };
    const groups = await streamsGroups(here.id);
    // A list without groups shows its channels at once.
    if (groups.length <= 1) return channels({ sourceId: here.id }, offset);
    const rows: Row[] = groups.map((g) => ({
      key: `sg:${here.id}:${g.name ?? ""}`,
      kind: "group",
      title: g.name ?? "Ungrouped",
      right: `${g.count} channel${g.count === 1 ? "" : "s"}`,
      open: {
        type: "channels",
        sourceId: here.id,
        group: g.name ?? "",
        label: g.name ?? "Ungrouped",
      },
    }));
    return { rows, total: rows.length };
  }
  if (here?.type === "channels")
    return channels({ sourceId: here.sourceId, group: here.group }, offset);

  switch (section) {
    case "video":
      if (text) return query("video", "all", offset, undefined, text);
      if (view === "folders")
        return offset === 0 ? roots(["video", "mixed"]) : { rows: [], total: 0 };
      return query("video", view === "all" ? "all" : "series", offset);
    case "music":
      if (text) return query("audio", "all", offset, undefined, text);
      return query(
        "audio",
        view === "tracks" ? "all" : view === "artists" ? "artists" : "albums",
        offset,
      );
    case "streams": {
      if (text) return channels({ search: text }, offset);
      if (offset > 0) return { rows: [], total: 0 };
      const sources = await streamsSources();
      const rows: Row[] = sources.map((s) => ({
        key: `s:${s.id}`,
        kind: "source",
        title: s.name,
        meta: s.location,
        right: `${s.count} channel${s.count === 1 ? "" : "s"}`,
        sourceId: s.id,
        open: { type: "source", id: s.id, label: s.name },
      }));
      return { rows, total: rows.length };
    }
    case "recent": {
      if (offset > 0) return { rows: [], total: 0 };
      const entries = await libraryRecent(200);
      const lower = text.toLowerCase();
      const rows = entries
        .map(recentRow)
        .filter((r) => !lower || r.title.toLowerCase().includes(lower));
      return { rows, total: rows.length };
    }
    case "folders":
      return offset === 0 ? roots(null) : { rows: [], total: 0 };
    case "playlists": {
      if (offset > 0) return { rows: [], total: 0 };
      const lists = await playlistList();
      const lower = text.toLowerCase();
      const rows: Row[] = lists
        .filter((p) => !lower || p.name.toLowerCase().includes(lower))
        .map((p) => ({
          key: `p:${p.id}`,
          kind: "playlist",
          title: p.name,
          meta: `${p.count} item${p.count === 1 ? "" : "s"}`,
          right: formatLength(p.duration),
          playlistId: p.id,
          open: { type: "playlist", id: p.id, label: p.name },
        }));
      return { rows, total: rows.length };
    }
  }
}

async function channels(
  where: { sourceId?: number; group?: string; search?: string },
  offset: number,
): Promise<Listing> {
  const page = await streamsList({ ...where, limit: PAGE, offset });
  return {
    total: page.total,
    rows: page.items.map((c) => ({
      key: `c:${c.id}`,
      kind: "channel",
      title: c.name,
      meta: c.group ?? undefined,
      sources: [c.url],
    })),
  };
}

/** Everything a row plays: its own source, or (for a row that opens) all that is inside. */
export async function resolveSources(row: Row): Promise<string[]> {
  if (row.sources) return row.sources;
  if (!row.open) return [];
  const sources: string[] = [];
  for (let offset = 0; offset < 5000; offset += PAGE) {
    const { rows, total } = await loadRows("video", undefined, [row.open], "", offset);
    for (const child of rows) {
      // One level only: a folder's files, an album's tracks — not its subfolders.
      if (child.sources) sources.push(...child.sources);
    }
    if (rows.length === 0 || offset + rows.length >= total) break;
  }
  return sources;
}

/**
 * What "play now" plays for a row in a listing: inside an album, a series, a playlist, or a
 * folder it plays on from that row, so the rest follows in order.
 */
export function playFrom(rows: Row[], row: Row, trail: Crumb[]): string[] | null {
  const here = trail[trail.length - 1];
  if (!row.sources || !here || here.type === "source" || here.type === "channels") return null;
  const index = rows.indexOf(row);
  if (index < 0) return null;
  return rows.slice(index).flatMap((r) => r.sources ?? []);
}
