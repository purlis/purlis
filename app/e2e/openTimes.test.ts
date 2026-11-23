// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { OPEN_HANG_MS, opensVerdict } from "./openTimes";

const steady = (n: number, ms: number): number[] => Array.from({ length: n }, () => ms);

describe("the fifty-sessions opening's times", () => {
  it("pass when a few opens are slow on a loaded runner, as the four runs of 2026-11-23 were", () => {
    for (const spike of [8063, 8302, 8052, 8822]) {
      expect(opensVerdict([...steady(49, 3400), spike])).toBeNull();
    }
    expect(opensVerdict([...steady(46, 3400), 8100, 8300, 8800, 9000])).toBeNull();
  });

  it("fail when every open got slower, though none is over the old per-open line", () => {
    expect(opensVerdict(steady(50, 5600))).toContain("a start is slower than it was");
  });

  it("fail when more than one open in ten is slow", () => {
    expect(opensVerdict([...steady(44, 3400), ...steady(6, 8600)])).toContain(
      "too many opens were slow",
    );
  });

  it("fail on one open that hung, and name its time", () => {
    const said = opensVerdict([...steady(49, 3400), OPEN_HANG_MS + 1]);
    expect(said).toContain("an open hung");
    expect(said).toContain(String(OPEN_HANG_MS + 1));
  });

  it("do not judge an opening that timed nothing", () => {
    expect(opensVerdict([])).toContain("no open was timed");
  });
});
