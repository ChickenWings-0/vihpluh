import { describe, expect, it } from "vitest";
import { formatTime, formatTimePrecise, parseTime } from "./time";

describe("formatTime", () => {
  it("formats minutes and hours", () => {
    expect(formatTime(0)).toBe("0:00");
    expect(formatTime(7.9)).toBe("0:07");
    expect(formatTime(754)).toBe("12:34");
    expect(formatTime(4354)).toBe("1:12:34");
  });

  it("treats bad input as zero", () => {
    expect(formatTime(-3)).toBe("0:00");
    expect(formatTime(Number.NaN)).toBe("0:00");
  });
});

describe("precise times", () => {
  it("formats milliseconds", () => {
    expect(formatTimePrecise(721.25)).toBe("12:01.250");
    expect(formatTimePrecise(3600.9996)).toBe("1:00:01.000");
  });
  it("parses what it formats, and plain seconds", () => {
    expect(parseTime("12:01.250")).toBeCloseTo(721.25);
    expect(parseTime("1:00:01")).toBe(3601);
    expect(parseTime("95.5")).toBe(95.5);
    expect(parseTime("abc")).toBeNull();
    expect(parseTime("1:2:3:4")).toBeNull();
  });
});
