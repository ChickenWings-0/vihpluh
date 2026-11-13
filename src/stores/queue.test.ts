import { describe, expect, it } from "vitest";
import type { QueueItem } from "@/ipc/bindings/QueueItem";
import { formatTotal, rangeIds } from "./queue";

const item = (id: string, duration: number | null = null): QueueItem => ({
  id,
  source: id,
  isUrl: false,
  title: id,
  duration,
  mediaKey: null,
  origin: "user",
  originRef: null,
  error: null,
});

describe("queue store helpers", () => {
  it("selects ranges in list order from either direction", () => {
    const items = ["a", "b", "c", "d"].map((id) => item(id));
    expect(rangeIds(items, "b", "d")).toEqual(["b", "c", "d"]);
    expect(rangeIds(items, "d", "b")).toEqual(["b", "c", "d"]);
    expect(rangeIds(items, null, "c")).toEqual(["c"]);
  });

  it("formats the total duration, ignoring unknown lengths", () => {
    expect(formatTotal([item("a", 3600 * 7 + 42 * 60), item("b")])).toBe("7 h 42 m+");
    expect(formatTotal([item("a", 90)])).toBe("2 m");
    expect(formatTotal([item("a")])).toBe("");
  });
});
