import { describe, expect, it } from "vitest";
import type { Bookmark } from "@/ipc/bindings/Bookmark";
import type { Chapter } from "@/ipc/bindings/Chapter";
import { currentMarkerIndex, mergeMarkers, stepMarker } from "./markers";

const chapters: Chapter[] = [
  { index: 0, title: "Cold Open", time: 0 },
  { index: 1, title: "Opening", time: 295 },
  { index: 2, title: "Part A", time: 385 },
];
const bookmark = (id: number, position: number, label: string): Bookmark => ({
  id,
  mediaKey: "f:1",
  position,
  label,
  createdAt: 0,
});
const bookmarks = [bookmark(7, 1421, "Bookmark 2"), bookmark(3, 192, "That one shot")];

describe("mergeMarkers", () => {
  it("interleaves chapters and bookmarks in time order", () => {
    const all = mergeMarkers(chapters, bookmarks);
    expect(all.map((m) => `${m.kind}:${m.label}`)).toEqual([
      "chapter:Cold Open",
      "bookmark:That one shot",
      "chapter:Opening",
      "chapter:Part A",
      "bookmark:Bookmark 2",
    ]);
  });

  it("filters, and puts a chapter before a bookmark at the same time", () => {
    expect(mergeMarkers(chapters, bookmarks, "chapters")).toHaveLength(3);
    expect(mergeMarkers(chapters, bookmarks, "bookmarks").map((m) => m.key)).toEqual(["b3", "b7"]);
    const tie = mergeMarkers(chapters, [bookmark(1, 295, "x")]);
    expect(tie.map((m) => m.key)).toEqual(["c0", "c1", "b1", "c2"]);
  });
});

describe("currentMarkerIndex", () => {
  const all = mergeMarkers(chapters, bookmarks);
  it("is the last marker at or before the position", () => {
    expect(currentMarkerIndex(all, 0)).toBe(0);
    expect(currentMarkerIndex(all, 191.9)).toBe(0);
    expect(currentMarkerIndex(all, 192)).toBe(1);
    expect(currentMarkerIndex(all, 5000)).toBe(4);
    expect(currentMarkerIndex(mergeMarkers([], bookmarks), 10)).toBe(-1);
  });
});

describe("stepMarker", () => {
  const all = mergeMarkers(chapters, bookmarks);
  it("goes forward to the next marker of either kind", () => {
    expect(stepMarker(all, 10, 1)?.label).toBe("That one shot");
    expect(stepMarker(all, 192, 1)?.label).toBe("Opening");
    expect(stepMarker(all, 1421, 1)).toBeNull();
  });

  it("goes back to this marker's start, or to the one before when already near it", () => {
    expect(stepMarker(all, 250, -1)?.label).toBe("That one shot");
    expect(stepMarker(all, 193, -1)?.label).toBe("Cold Open");
    expect(stepMarker(all, 1, -1)?.label).toBe("Cold Open");
    expect(stepMarker(mergeMarkers([], bookmarks), 10, -1)).toBeNull();
  });
});
