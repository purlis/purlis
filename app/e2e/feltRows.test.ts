// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  BURSTS,
  FRAME_P95_MS,
  KEPT,
  burstOf,
  P95_SAMPLES,
  ROWS,
  frameTime,
  heldTo,
  lateAnswers,
  switchPlan,
} from "./feltRows.js";

/** `count` samples: quick ones, and the slowest last. */
function samples(count: number, quick: number, slowest: number[]): number[] {
  return [...Array.from({ length: count - slowest.length }, () => quick), ...slowest];
}

describe("heldTo", () => {
  it("reads the p95 of forty samples as the second-worst one, so one bad sample is let go", () => {
    const one = heldTo(ROWS.switchAtFiftyBusy, samples(40, 40, [400]));
    expect(one.p95).toBe(40);
    expect(one.met).toBe(true);
    const two = heldTo(ROWS.switchAtFiftyBusy, samples(40, 40, [101, 400]));
    expect(two.p95).toBe(101);
    expect(two.met).toBe(false);
  });

  it("is never met on fewer samples than a p95 needs, whose p95 would be their worst", () => {
    expect(P95_SAMPLES).toBe(40);
    expect(heldTo(ROWS.dialogOpen, samples(39, 30, [])).met).toBe(false);
    expect(heldTo(ROWS.echoAfterASwitch, []).met).toBe(false);
  });

  it("carries the row's budget beside its numbers, for the run's verdict line", () => {
    const row = heldTo(ROWS.dialogOpen, samples(40, 30, []));
    expect(row).toMatchObject({ budgetMs: 100, p50: 30, p95: 30, worst: 30, samples: 40 });
  });
});

describe("frameTime", () => {
  it("holds frames to 33 ms, the spec's limit", () => {
    expect(FRAME_P95_MS).toBe(33);
  });

  it("is met at a 60 fps display's frames with one dropped now and then", () => {
    const gaps = [
      ...Array.from({ length: 590 }, () => 16.7),
      ...Array.from({ length: 10 }, () => 33.3),
    ];
    expect(frameTime(gaps)).toMatchObject({ met: true, stalls: 0 });
  });

  it("is missed at 30 fps throughout, a frame of 33.4 ms being past 33", () => {
    expect(frameTime(Array.from({ length: 300 }, () => 33.4)).met).toBe(false);
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

describe("switchPlan", () => {
  const plan = switchPlan(P95_SAMPLES, 6, 43);

  it("never switches back to one of the last three chats shown, which kept terminals keep", () => {
    const shown: string[] = [];
    for (const step of plan) {
      const target = `target ${step.target}`;
      shown.push(target);
      for (const away of step.away) shown.push(`streaming ${away}`);
      expect(shown.slice(-KEPT)).not.toContain(target);
      shown.push(target);
    }
  });

  it("rotates over more chats than are kept", () => {
    expect(new Set(plan.map((step) => step.target)).size).toBeGreaterThan(KEPT);
  });

  it("bursts each target before switching to it, 2 MB and 13 MB in turn, half each", () => {
    expect(plan).toHaveLength(P95_SAMPLES);
    expect(plan.map((step) => step.loops).slice(0, 4)).toEqual([
      BURSTS["2 MB"],
      BURSTS["13 MB"],
      BURSTS["2 MB"],
      BURSTS["13 MB"],
    ]);
    expect(plan.filter((step) => step.loops === BURSTS["13 MB"])).toHaveLength(P95_SAMPLES / 2);
  });

  it("sends a target the same burst every time, the one its harness was started with", () => {
    for (const step of plan) expect(step.loops).toBe(burstOf(step.target));
  });

  it("refuses to rotate over no more chats than are kept, or over an odd number", () => {
    expect(() => switchPlan(P95_SAMPLES, KEPT, 43)).toThrow();
    expect(() => switchPlan(P95_SAMPLES, 5, 43)).toThrow();
  });
});

describe("lateAnswers", () => {
  const log = [
    '{"kind":"answer","command":"startChat","ms":1250,"at":1000,"hot":3}',
    '{"kind":"stall","ms":340,"at":2000,"hot":12,"front":"steward 3","opening":true}',
    '{"kind":"answer","command":"openedChats","ms":1800,"at":3000,"hot":40}',
    '{"kind":"answer","command":"planeSidebar","ms":1100,"at":4500,"hot":40}',
    '{"kind":"answ',
    "",
  ].join("\n");
  const phases = [
    { name: "L11", from: 1500, to: 3500 },
    { name: "L14", from: 5000, to: 6000 },
  ];

  it("counts the answers of 1 s or longer written inside a measured phase, and lists them", () => {
    const read = lateAnswers(log, phases);
    expect(read.met).toBe(false);
    expect(read.answers).toEqual([{ command: "openedChats", ms: 1800, hot: 40, phase: "L11" }]);
    expect(read.stalls).toBe(1);
  });

  it("leaves out what the bench's own setup asked between phases", () => {
    expect(lateAnswers(log, [{ name: "L14", from: 5000, to: 6000 }]).met).toBe(true);
  });

  it("is met by a run that wrote no answer at all, a stall being evidence and not the row", () => {
    expect(
      lateAnswers('{"kind":"stall","ms":340,"at":2000,"hot":1,"front":null,"opening":false}\n', [
        { name: "L11", from: 0, to: 3000 },
      ]),
    ).toEqual({ met: true, logWritten: true, answers: [], stalls: 1 });
  });

  it("is missed where the window wrote no log, which says nothing about its answers", () => {
    expect(lateAnswers(undefined, phases)).toEqual({
      met: false,
      logWritten: false,
      answers: [],
      stalls: 0,
    });
  });
});
