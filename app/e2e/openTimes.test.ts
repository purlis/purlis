// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  OPEN_HANG_MS,
  OPEN_MEDIAN_MS,
  type Open,
  SWAP_GROWTH_MB,
  judgeOpens,
  opensVerdict,
  parseSwapUsedMb,
  runnerSummary,
} from "./openTimes";

const steady = (n: number, ms: number): number[] => Array.from({ length: n }, () => ms);

describe("the fifty-sessions opening's times", () => {
  it("pass when a few opens are slow on a loaded runner, as the four runs of 2026-11-23 were", () => {
    for (const spike of [8063, 8302, 8052, 8822]) {
      expect(opensVerdict([...steady(49, 3400), spike])).toBeNull();
    }
    expect(opensVerdict([...steady(46, 3400), 8100, 8300, 8800, 9000])).toBeNull();
  });

  it("pass the two attempts of main's run 45144506452, whose medians were 5,586 and 5,545 ms", () => {
    expect(opensVerdict([...steady(24, 4600), ...steady(21, 5586), ...steady(5, 6797)])).toBeNull();
    expect(opensVerdict([...steady(24, 4700), ...steady(21, 5545), ...steady(5, 6191)])).toBeNull();
  });

  it("fail when every open got slower than the median limit, though none is over the old per-open line", () => {
    expect(opensVerdict(steady(50, OPEN_MEDIAN_MS))).toBeNull();
    expect(opensVerdict(steady(50, OPEN_MEDIAN_MS + 1))).toContain("a start is slower than it was");
    expect(opensVerdict(steady(50, 7500))).toContain("a start is slower than it was");
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

const open = (ms: number, swapMb: number | null): Open => ({
  ms,
  runner: { swapMb, freeMb: 400, load: 3.5 },
});
/** Fifty opens of `ms`, swap rising by `rise` MB over the run from 100 MB. */
const run = (ms: number, rise: number): Open[] =>
  Array.from({ length: 50 }, (_, i) => open(ms, 100 + Math.round((rise * i) / 49)));

describe("judging the opens beside the runner", () => {
  it("holds a quick run, swapping or not", () => {
    expect(judgeOpens(run(3400, 0)).state).toBe("held");
    expect(judgeOpens(run(3400, 4000)).state).toBe("held");
  });

  it("fails slow opens on a runner whose swap stayed flat", () => {
    const judged = judgeOpens(run(OPEN_MEDIAN_MS + 500, SWAP_GROWTH_MB));
    expect(judged.state).toBe("failed");
    expect(judged.said).toContain("a start is slower than it was");
  });

  it("is inconclusive on slow opens when swap grew, and says the runner was swapping", () => {
    const judged = judgeOpens(run(OPEN_MEDIAN_MS + 500, SWAP_GROWTH_MB + 1));
    expect(judged.state).toBe("inconclusive");
    expect(judged.said).toContain("the runner was swapping");
    expect(judged.said).toContain("runner: swap 100 MB");
  });

  it("still fails an open over the hang limit on a swapping runner", () => {
    const opens = run(5000, 4000);
    opens[10] = open(OPEN_HANG_MS + 1, 900);
    expect(judgeOpens(opens).state).toBe("failed");
  });

  it("does not call a run swapping when the platform gave no swap figure", () => {
    const opens = Array.from({ length: 50 }, () => open(OPEN_MEDIAN_MS + 500, null));
    expect(judgeOpens(opens).state).toBe("failed");
    expect(runnerSummary(opens)).toContain("swap n/a -> n/a");
  });

  it("fails an opening that timed nothing", () => {
    expect(judgeOpens([]).state).toBe("failed");
    expect(runnerSummary([])).toBe("runner: not sampled");
  });

  it("reads swap used from sysctl vm.swapusage", () => {
    expect(
      parseSwapUsedMb(
        "vm.swapusage: total = 2048.00M  used = 1180.25M  free = 867.75M  (encrypted)",
      ),
    ).toBe(1180.25);
    expect(parseSwapUsedMb("vm.swapusage: total = 4.00G  used = 1.50G  free = 2.50G")).toBe(1536);
    expect(parseSwapUsedMb("")).toBeNull();
  });
});
