import { beforeEach, describe, expect, it } from "vitest";
import { useOsdStore } from "./osd";

describe("toasts", () => {
  beforeEach(() => useOsdStore.setState({ toasts: [] }));

  it("replaces a toast of the same group in place", () => {
    const { pushToast } = useOsdStore.getState();
    pushToast({ kind: "info", text: "other" });
    pushToast({ kind: "error", text: "Can't play a", group: "q" });
    pushToast({ kind: "error", text: "Can't play b", group: "q" });
    pushToast({ kind: "error", text: "Stopped after 3", group: "q" });
    expect(useOsdStore.getState().toasts.map((t) => t.text)).toEqual(["other", "Stopped after 3"]);
  });

  it("stacks ungrouped toasts up to the limit", () => {
    const { pushToast } = useOsdStore.getState();
    for (const text of ["1", "2", "3", "4"]) pushToast({ kind: "error", text });
    expect(useOsdStore.getState().toasts.map((t) => t.text)).toEqual(["2", "3", "4"]);
  });
});
