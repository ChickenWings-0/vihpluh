/**
 * Chapters and bookmarks as one timeline (DESIGN_SYSTEM §10.5): the merged list for the
 * Bookmarks tab, the row the playhead is in, and stepping to the next or previous marker.
 */
import type { Bookmark } from "@/ipc/bindings/Bookmark";
import type { Chapter } from "@/ipc/bindings/Chapter";

export type Marker =
  | { kind: "chapter"; key: string; time: number; label: string }
  | { kind: "bookmark"; key: string; time: number; label: string; id: number };

export type MarkerFilter = "all" | "chapters" | "bookmarks";

/** Chapters and bookmarks in time order; at the same time the chapter comes first. */
export function mergeMarkers(
  chapters: readonly Chapter[],
  bookmarks: readonly Bookmark[],
  filter: MarkerFilter = "all",
): Marker[] {
  const out: Marker[] = [];
  if (filter !== "bookmarks") {
    for (const c of chapters) {
      out.push({ kind: "chapter", key: `c${c.index}`, time: c.time, label: c.title });
    }
  }
  if (filter !== "chapters") {
    for (const b of bookmarks) {
      out.push({ kind: "bookmark", key: `b${b.id}`, time: b.position, label: b.label, id: b.id });
    }
  }
  return out.sort(
    (a, b) => a.time - b.time || (a.kind === b.kind ? 0 : a.kind === "chapter" ? -1 : 1),
  );
}

/** Index of the last marker at or before `position`, or −1 before the first. */
export function currentMarkerIndex(markers: readonly Marker[], position: number): number {
  let index = -1;
  for (let i = 0; i < markers.length; i++) {
    if ((markers[i]?.time ?? Number.POSITIVE_INFINITY) <= position + 0.001) index = i;
    else break;
  }
  return index;
}

/**
 * Where "next" / "previous marker" goes from `position`. Previous within the first seconds
 * of a marker goes to the one before it; later, back to its start (like Previous in a queue).
 */
export function stepMarker(
  markers: readonly Marker[],
  position: number,
  direction: 1 | -1,
  restartAfter = 2,
): Marker | null {
  if (direction === 1) {
    return markers.find((m) => m.time > position + 0.5) ?? null;
  }
  const index = currentMarkerIndex(markers, position);
  if (index < 0) return null;
  const here = markers[index];
  if (here && position - here.time > restartAfter) return here;
  return markers[index - 1] ?? here ?? null;
}
