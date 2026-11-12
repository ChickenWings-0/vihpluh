import { beforeEach, describe, expect, it } from "vitest";
import { getPosition, interpolate, receiveTime, resetTime, setClock } from "./time";

describe("interpolate", () => {
  const sample = { pos: 10, dur: 100, cacheEnd: 0, receivedAt: 1000 };

  it("holds still when not advancing", () => {
    expect(interpolate(sample, 5000, false, 1)).toBe(10);
  });

  it("advances with elapsed time and speed", () => {
    expect(interpolate(sample, 1500, true, 1)).toBeCloseTo(10.5);
    expect(interpolate(sample, 1500, true, 2)).toBeCloseTo(11);
  });

  it("never runs past the duration", () => {
    expect(interpolate(sample, 1_000_000, true, 1)).toBe(100);
  });

  it("ignores receive times in the future", () => {
    expect(interpolate(sample, 900, true, 1)).toBe(10);
  });
});

describe("clock", () => {
  beforeEach(() => resetTime());

  it("re-anchors on a rate change so the position does not jump", () => {
    receiveTime({ pos: 0, dur: 600, cacheEnd: 0, at: 0 }, 0);
    setClock(true, 1, 0);
    expect(getPosition(2000)).toBeCloseTo(2);
    setClock(true, 2, 2000);
    expect(getPosition(2000)).toBeCloseTo(2);
    expect(getPosition(3000)).toBeCloseTo(4);
    setClock(false, 2, 3000);
    expect(getPosition(9000)).toBeCloseTo(4);
  });
});
