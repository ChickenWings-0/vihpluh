import { describe, expect, it } from "vitest";
import { cleanSource } from "@/actions/definitions/library";
import type { LibraryMedia } from "@/ipc/bindings/LibraryMedia";
import {
  type Crumb,
  episodeLabel,
  formatLength,
  groupRow,
  mediaRow,
  playFrom,
  type Row,
  recentRow,
} from "./model";

const media = (over: Partial<LibraryMedia>): LibraryMedia => ({
  id: 1,
  path: "D:/Shows/Show S01E02.mkv",
  kind: "video",
  title: "Show S01E02",
  duration: 1420,
  seriesKey: "show",
  season: 1,
  episode: 2,
  artist: null,
  albumArtist: null,
  album: null,
  trackNo: null,
  discNo: null,
  year: null,
  position: null,
  finished: false,
  ...over,
});

describe("formatLength", () => {
  it("is exact for short things and rounds long ones to minutes", () => {
    expect(formatLength(185)).toBe("3:05");
    expect(formatLength(47 * 60 + 10)).toBe("47 m");
    expect(formatLength(2 * 3600 + 44 * 60)).toBe("2 h 44 m");
    expect(formatLength(null)).toBeUndefined();
    expect(formatLength(0)).toBeUndefined();
  });
});

describe("rows", () => {
  it("labels episodes and shows resume progress", () => {
    expect(episodeLabel(1, 2)).toBe("S01E02");
    expect(episodeLabel(null, 5)).toBe("Episode 05");
    expect(episodeLabel(1, null)).toBeUndefined();
    const row = mediaRow(media({ position: 355 }), false);
    expect(row).toMatchObject({ meta: "show · S01E02", right: "24 m", sources: [media({}).path] });
    expect(row.progress).toBeCloseTo(0.25);
    // Inside its series the series name is not repeated; a finished file has no progress line.
    const inside = mediaRow(media({ position: 355, finished: true }), true);
    expect(inside.meta).toBe("S01E02");
    expect(inside.progress).toBeUndefined();
    expect(inside.finished).toBe(true);
  });

  it("describes music by track, artist, and album", () => {
    const track = media({
      kind: "audio",
      title: "So What",
      seriesKey: null,
      season: null,
      episode: null,
      artist: "Miles Davis",
      album: "Kind of Blue",
      trackNo: 1,
      duration: 562,
    });
    expect(mediaRow(track, false).meta).toBe("Miles Davis · Kind of Blue");
    expect(mediaRow(track, true).meta).toBe("Track 1 · Miles Davis");
    expect(mediaRow(track, true).right).toBe("9:22");
  });

  it("names untagged groups and counts in the right noun", () => {
    const album = groupRow(
      { key: "\u001f", title: "", subtitle: null, year: null, count: 1 },
      "audio",
      "albums",
    );
    expect(album).toMatchObject({ title: "Unknown album", right: "1 track" });
    const series = groupRow(
      { key: "show", title: "show", subtitle: null, year: null, count: 12 },
      "video",
      "series",
    );
    expect(series.right).toBe("12 episodes");
    expect(series.open).toMatchObject({ type: "group", key: "show", view: "series" });
  });

  it("shows a recent file's folder and a stream's address", () => {
    const base = {
      mediaKey: "f:1",
      title: "t",
      duration: 100,
      position: 50,
      finished: false,
      lastPlayedAt: 0,
    };
    const file = recentRow({ ...base, source: "D:\\Shows\\Severance\\e01.mkv" });
    expect(file).toMatchObject({
      meta: "Severance",
      path: "D:\\Shows\\Severance\\e01.mkv",
      progress: 0.5,
    });
    const stream = recentRow({ ...base, mediaKey: "u:1", source: "https://example.com/live.m3u8" });
    expect(stream.meta).toBe("https://example.com/live.m3u8");
    expect(stream.path).toBeUndefined();
  });
});

describe("playFrom", () => {
  const rows: Row[] = ["a", "b", "c"].map((n) => ({
    key: n,
    kind: "media",
    title: n,
    sources: [n],
  }));
  const album: Crumb = { type: "group", kind: "audio", view: "albums", key: "k", label: "Album" };

  it("plays on from the chosen row inside an album, a series, a folder, or a playlist", () => {
    expect(playFrom(rows, rows[1] as Row, [album])).toEqual(["b", "c"]);
    expect(playFrom(rows, rows[0] as Row, [{ type: "dir", path: "D:/x", label: "x" }])).toEqual([
      "a",
      "b",
      "c",
    ]);
  });

  it("plays just the row at the top of a section and among channels", () => {
    expect(playFrom(rows, rows[1] as Row, [])).toBeNull();
    expect(
      playFrom(rows, rows[1] as Row, [{ type: "channels", sourceId: 1, group: "", label: "g" }]),
    ).toBeNull();
    const group: Row = { key: "g", kind: "group", title: "g", open: album };
    expect(playFrom([group], group, [album])).toBeNull();
  });
});

describe("cleanSource", () => {
  it("trims and unquotes what was typed or pasted", () => {
    expect(cleanSource('  "D:\\Films\\a b.mkv" ')).toBe("D:\\Films\\a b.mkv");
    expect(cleanSource(" https://example.com/x ")).toBe("https://example.com/x");
    expect(cleanSource("   ")).toBeNull();
  });
});
