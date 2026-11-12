import { describe, expect, it } from "vitest";
import type { PlayerState } from "@/ipc/bindings/PlayerState";
import { mergePatch } from "./player";

const base = { rev: 4, volume: 100, muted: false } as unknown as PlayerState;

describe("mergePatch", () => {
  it("needs a snapshot first", () => {
    expect(mergePatch(null, { rev: 1, volume: 50 }).result).toBe("no-base");
  });

  it("applies the next revision as a shallow merge", () => {
    const { state, result } = mergePatch(base, { rev: 5, volume: 80 });
    expect(result).toBe("applied");
    expect(state?.volume).toBe(80);
    expect(state?.muted).toBe(false);
    expect(state?.rev).toBe(5);
  });

  it("ignores old or repeated revisions", () => {
    expect(mergePatch(base, { rev: 4, volume: 1 }).result).toBe("stale");
    expect(mergePatch(base, { rev: 2, volume: 1 }).result).toBe("stale");
  });

  it("reports a gap so the caller re-fetches", () => {
    const { state, result } = mergePatch(base, { rev: 7, volume: 1 });
    expect(result).toBe("gap");
    expect(state).toBe(base);
  });
});
