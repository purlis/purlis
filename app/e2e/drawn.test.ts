// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { FRAMES_NEEDED, noFramesSaid } from "./drawn";

describe("a window that paints no frames (#7738)", () => {
  it("is let through once it has painted the frames a spec needs", () => {
    expect(noFramesSaid({ frames: FRAMES_NEEDED, waitedMs: 33, visibility: "visible" })).toBe(
      undefined,
    );
  });

  it("fails with what it painted, its visibility and what to change on the Mac", () => {
    const said = noFramesSaid({ frames: 0, waitedMs: 5000.4, visibility: "hidden" });
    expect(said).toContain("painted 0 frames in 5000 ms");
    expect(said).toContain("document.visibilityState: hidden");
    expect(said).toContain("behind the lock screen");
    expect(said).toContain('"The outcome bars on a self-hosted Mac"');
  });

  it("counts a single callback as not painting, since it may be the one run as the page showed", () => {
    expect(noFramesSaid({ frames: 1, waitedMs: 5000, visibility: "visible" })).toContain(
      "painted 1 frames",
    );
  });
});
