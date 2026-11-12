import { describe, expect, it } from "vitest";
import { isAppError } from "./transport";

describe("isAppError", () => {
  it("accepts the core's error shape", () => {
    expect(isAppError({ code: "not_found", message: "No such file" })).toBe(true);
    expect(isAppError({ code: "mpv", message: "failed", detail: "loadfile" })).toBe(true);
  });

  it("rejects anything else", () => {
    expect(isAppError(null)).toBe(false);
    expect(isAppError("boom")).toBe(false);
    expect(isAppError(new Error("boom"))).toBe(false);
    expect(isAppError({ code: 1, message: "x" })).toBe(false);
  });
});
