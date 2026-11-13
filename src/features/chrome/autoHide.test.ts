import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AutoHide, type AutoHideInput, REVEAL_MS, type Visibility } from "./autoHide";

const base: AutoHideInput = {
  playing: true,
  fullscreen: false,
  autohideWindowed: true,
  held: false,
  zen: false,
  topFocused: false,
};

describe("AutoHide", () => {
  let seen: Visibility[];
  let ctl: AutoHide;
  const last = () => seen[seen.length - 1];

  beforeEach(() => {
    vi.useFakeTimers();
    seen = [];
    ctl = new AutoHide(
      2000,
      (v) => seen.push(v),
      () => Date.now(),
    );
  });
  afterEach(() => {
    ctl.dispose();
    vi.useRealTimers();
  });

  it("hides after the delay while playing and shows on pointer movement", () => {
    ctl.setInput(base);
    expect(last()).toEqual({ bottom: true, top: true, cursorHidden: false });
    vi.advanceTimersByTime(1999);
    expect(last()?.bottom).toBe(true);
    vi.advanceTimersByTime(1);
    expect(last()).toEqual({ bottom: false, top: false, cursorHidden: true });
    ctl.pointerMove(300);
    expect(last()).toEqual({ bottom: true, top: true, cursorHidden: false });
  });

  it("restarts the delay on every movement", () => {
    ctl.setInput(base);
    vi.advanceTimersByTime(1500);
    ctl.pointerMove(300);
    vi.advanceTimersByTime(1500);
    expect(last()?.bottom).toBe(true);
    vi.advanceTimersByTime(500);
    expect(last()?.bottom).toBe(false);
  });

  it("never hides while paused", () => {
    ctl.setInput({ ...base, playing: false });
    vi.advanceTimersByTime(10_000);
    expect(last()?.bottom).toBe(true);
  });

  it("pausing while hidden brings the bars back", () => {
    ctl.setInput(base);
    vi.advanceTimersByTime(2000);
    expect(last()?.bottom).toBe(false);
    ctl.setInput({ ...base, playing: false });
    expect(last()?.bottom).toBe(true);
  });

  it("never hides while held, and starts counting again when released", () => {
    ctl.setInput({ ...base, held: true });
    vi.advanceTimersByTime(10_000);
    expect(last()?.bottom).toBe(true);
    ctl.setInput(base);
    vi.advanceTimersByTime(2000);
    expect(last()?.bottom).toBe(false);
  });

  it("respects autohide_in_windowed = false, but still hides in fullscreen", () => {
    ctl.setInput({ ...base, autohideWindowed: false });
    vi.advanceTimersByTime(10_000);
    expect(last()?.bottom).toBe(true);
    ctl.setInput({ ...base, autohideWindowed: false, fullscreen: true });
    vi.advanceTimersByTime(2000);
    expect(last()?.bottom).toBe(false);
  });

  it("in fullscreen, movement shows only the bottom bar; the top edge shows the top bar", () => {
    ctl.setInput({ ...base, fullscreen: true });
    ctl.pointerMove(500);
    expect(last()).toEqual({ bottom: true, top: false, cursorHidden: false });
    ctl.pointerMove(10);
    expect(last()).toEqual({ bottom: true, top: true, cursorHidden: false });
    ctl.pointerLeave();
    expect(last()?.top).toBe(false);
  });

  it("in fullscreen, keyboard focus in the top bar shows it", () => {
    ctl.setInput({ ...base, fullscreen: true, topFocused: true });
    expect(last()?.top).toBe(true);
  });

  it("zen hides everything and ignores the pointer", () => {
    ctl.setInput({ ...base, zen: true, playing: false });
    expect(last()).toEqual({ bottom: false, top: false, cursorHidden: true });
    ctl.pointerMove(5);
    expect(last()).toEqual({ bottom: false, top: false, cursorHidden: true });
    ctl.setInput({ ...base, playing: false });
    expect(last()?.bottom).toBe(true);
  });

  it("reveal shows the bottom bar briefly without counting as activity", () => {
    ctl.setInput(base);
    vi.advanceTimersByTime(2000);
    expect(last()?.bottom).toBe(false);
    ctl.reveal();
    expect(last()?.bottom).toBe(true);
    expect(last()?.top).toBe(false);
    vi.advanceTimersByTime(REVEAL_MS);
    expect(last()?.bottom).toBe(false);
  });

  it("only reports changes", () => {
    ctl.setInput({ ...base, playing: false });
    ctl.setInput({ ...base, playing: false });
    ctl.pointerMove(300);
    expect(seen).toHaveLength(1);
  });
});
