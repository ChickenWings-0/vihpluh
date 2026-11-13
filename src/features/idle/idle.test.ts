import { describe, expect, it } from "vitest";
import type { RecentEntry } from "@/ipc/bindings/RecentEntry";
import { folderOf, splitRecent } from "./IdleScreen";

const entry = (source: string, position: number, finished = false): RecentEntry => ({
  mediaKey: source,
  source,
  title: source,
  duration: 1000,
  position,
  finished,
  lastPlayedAt: 0,
});

describe("idle history", () => {
  it("finds the folder of local paths", () => {
    expect(folderOf("D:\\Shows\\S2\\e1.mkv")).toBe("D:\\Shows\\S2");
    expect(folderOf("/home/me/a.mkv")).toBe("/home/me");
    expect(folderOf("a.mkv")).toBeNull();
  });

  it("lists unfinished files and distinct folders, skipping URLs", () => {
    const { resume, folders } = splitRecent([
      entry("D:\\S\\e3.mkv", 300),
      entry("D:\\S\\e2.mkv", 0, true),
      entry("https://example.com/live.m3u8", 500),
      entry("E:\\Films\\f.mkv", 0),
      entry("F:\\A\\one.mkv", 61),
      entry("G:\\B\\two.mkv", 59),
    ]);
    expect(resume.map((r) => r.source)).toEqual([
      "D:\\S\\e3.mkv",
      "https://example.com/live.m3u8",
      "F:\\A\\one.mkv",
    ]);
    expect(folders).toEqual(["D:\\S", "E:\\Films", "F:\\A"]);
  });
});
