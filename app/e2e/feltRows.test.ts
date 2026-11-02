// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { ROWS, frameTime, heldTo, lateAnswers } from "./feltRows.js";

/** Twenty samples: nineteen quick ones and the slowest last. */
function twenty(quick: number, slowest: number[]): number[] {
  return [...Array.from({ length: 20 - slowest.length }, () => quick), ...slowest];
}

describe("heldTo", () => {
  it("reads the p95 of twenty samples as the second-worst one, and meets a budget it is at", () => {
    const row = heldTo(ROWS.switchAtFiftyBusy, twenty(40, [100, 400]));
    // floor(0.95 × 20) = 19th of 0..19 is the worst; that is what twenty samples allow.
    expect(row.p95).toBe(400);
    expect(row.met).toBe(false);
    expect(heldTo(ROWS.switchAtFiftyBusy, twenty(40, [100])).met).toBe(true);
  });

  it("is never met on no samples at all", () => {
    expect(heldTo(ROWS.echoAfterASwitch, []).met).toBe(false);
  });

  it("carries the row's budget beside its numbers, for the run's verdict line", () => {
    const row = heldTo(ROWS.dialogOpen, twenty(30, []));
    expect(row).toMatchObject({ budgetMs: 100, p50: 30, p95: 30, worst: 30, samples: 20 });
  });
});

describe("frameTime", () => {
  it("is met at a 60 fps display's frames with one dropped now and then", () => {
    const gaps = [
      ...Array.from({ length: 590 }, () => 16.7),
      ...Array.from({ length: 10 }, () => 33.3),
    ];
    expect(frameTime(gaps)).toMatchObject({ met: true, stalls: 0 });
  });

  it("is missed by a single stall of 200 ms, however smooth the rest", () => {
    const gaps = [...Array.from({ length: 599 }, () => 16.7), 200];
    expect(frameTime(gaps)).toMatchObject({ met: false, stalls: 1, longestMs: 200 });
  });

  it("is missed when more than one frame in twenty is late", () => {
    const gaps = [
      ...Array.from({ length: 90 }, () => 16.7),
      ...Array.from({ length: 10 }, () => 50),
    ];
    expect(frameTime(gaps).met).toBe(false);
  });
});

describe("lateAnswers", () => {
  const log = [
    '{"kind":"answer","command":"planeSidebar","ms":1250,"at":1000,"hot":3}',
    '{"kind":"stall","ms":340,"at":2000,"hot":12,"front":"steward 3","opening":true}',
    '{"kind":"answer","command":"openedChats","ms":1800,"at":3000,"hot":40}',
    '{"kind":"answ',
    "",
  ].join("\n");

  it("counts the answers of 1 s or longer written since the run began, and lists them", () => {
    const read = lateAnswers(log, 1500);
    expect(read.met).toBe(false);
    expect(read.answers).toEqual([{ command: "openedChats", ms: 1800, hot: 40 }]);
    expect(read.stalls).toBe(1);
  });

  it("is met by a run that wrote no answer at all, a stall being evidence and not the row", () => {
    expect(
      lateAnswers('{"kind":"stall","ms":340,"at":2000,"hot":1,"front":null,"opening":false}\n', 0),
    ).toEqual({
      met: true,
      answers: [],
      stalls: 1,
    });
  });

  it("is met where the window wrote no log", () => {
    expect(lateAnswers(undefined, 0)).toEqual({ met: true, answers: [], stalls: 0 });
  });
});
