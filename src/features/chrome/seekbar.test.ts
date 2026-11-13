import { describe, expect, it } from "vitest";
import { bucketStep } from "./SeekBar";

// Must agree with `media::thumbs::bucket_step` in the core (same cases as its test).
describe("thumbnail buckets", () => {
  it("matches the core's step", () => {
    expect(bucketStep(150)).toBe(2);
    expect(bucketStep(1420)).toBe(5);
    expect(bucketStep(3 * 3600)).toBe(30);
    expect(bucketStep(Number.NaN)).toBe(2);
  });
});
