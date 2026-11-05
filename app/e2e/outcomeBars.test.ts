// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  OPEN_BUDGET_MS,
  OPEN_GUARD_MEDIAN_MS,
  USE_BUDGET_MS,
  aPersonsTime,
  harnessUnderTest,
  openJudged,
  openVerdict,
  reasonProblem,
  useVerdict,
} from "./outcomeBars.js";

describe("the 300 ms bar", () => {
  it("is met only with the p95 under the budget", () => {
    const quick = Array.from({ length: 20 }, () => 100);
    expect(openVerdict(quick)).toMatchObject({ met: true, p95: 100 });
    expect(openVerdict([...quick.slice(2), OPEN_BUDGET_MS, OPEN_BUDGET_MS])).toMatchObject({
      met: false,
      p95: OPEN_BUDGET_MS,
    });
  });

  it("says the number when it is missed", () => {
    const said = openVerdict(Array.from({ length: 20 }, () => 412)).said;
    expect(said).toContain("412 ms");
    expect(said).toContain(`${OPEN_BUDGET_MS} ms`);
  });

  it("refuses to judge no samples", () => {
    expect(() => openVerdict([])).toThrow(/no samples/);
  });
});

describe("how an L10 opening is judged where it is measured", () => {
  const of = (ms: number) => Array.from({ length: 20 }, () => ms);
  const spiky = [...of(OPEN_GUARD_MEDIAN_MS - 1).slice(2), 5000, 5000];

  it("holds a real machine to the 300 ms p95", () => {
    const here = { ci: false, platform: "darwin" };
    expect(openJudged(of(OPEN_BUDGET_MS - 1), here).met).toBe(true);
    expect(openJudged(of(OPEN_BUDGET_MS), here).met).toBe(false);
    expect(openJudged(of(OPEN_BUDGET_MS), here).said).toContain("p95");
  });

  it("holds macOS CI to the median guard, not to 300 ms", () => {
    const here = { ci: true, platform: "darwin" };
    expect(openJudged(spiky, here).met).toBe(true);
    expect(openJudged(of(OPEN_GUARD_MEDIAN_MS), here).met).toBe(false);
    expect(openJudged(of(OPEN_GUARD_MEDIAN_MS), here).said).toContain(
      `median ${OPEN_GUARD_MEDIAN_MS}.0 ms`,
    );
  });

  it("takes the middle opening, or the mean of the two in the middle of an even count", () => {
    const here = { ci: true, platform: "darwin" };
    expect(openJudged([100, 200, 900], here).said).toContain("median 200.0 ms");
    expect(openJudged([100, 200, 300, 900], here).said).toContain("median 250.0 ms");
  });

  it("does not read a median just under the guard as the guard's own number", () => {
    const here = { ci: true, platform: "darwin" };
    const verdict = openJudged([OPEN_GUARD_MEDIAN_MS - 0.5], here);
    expect(verdict.met).toBe(true);
    expect(verdict.said).toContain(`median ${OPEN_GUARD_MEDIAN_MS - 0.5} ms`);
  });

  it("sets no limit on Linux CI, where no run has measured one, and says so", () => {
    const verdict = openJudged(of(5000), { ci: true, platform: "linux" });
    expect(verdict.met).toBe(true);
    expect(verdict.said).toContain("no limit");
  });

  it("refuses to judge no samples anywhere", () => {
    expect(() => openJudged([], { ci: true, platform: "darwin" })).toThrow(/no samples/);
    expect(() => openJudged([], { ci: true, platform: "linux" })).toThrow(/no samples/);
  });
});

describe("the 2 minute bar", () => {
  it("adds what a person spends to what the window took", () => {
    // Three presses and ten keys, by the keystroke-level model's figures.
    expect(aPersonsTime({ presses: 3, characters: 10 })).toBe(3 * 2650 + 10 * 280);
  });

  it("is missed by a window quick enough for a robot but not for a person", () => {
    const verdict = useVerdict(USE_BUDGET_MS - 1000, { presses: 2, characters: 0 });
    expect(verdict.met).toBe(false);
    expect(verdict.said).toContain("2 presses");
  });

  it("is met when the window's wait and the person's together are under the budget", () => {
    expect(useVerdict(10_000, { presses: 10, characters: 40 }).met).toBe(true);
  });
});

describe("a skill's one-line reason", () => {
  it("accepts a sentence", () => {
    expect(reasonProblem("turned off in this workspace")).toBeUndefined();
  });

  it.each([
    ["", /says nothing/],
    ["   ", /says nothing/],
    ["first\nsecond", /more than one line/],
    ["Left out", /only names the state/],
    ["Off", /only names the state/],
    ["Hidden by the router", /only names the state/],
  ])("refuses %j", (said, problem) => {
    expect(reasonProblem(said)).toMatch(problem);
  });
});

describe("the harness a run is about", () => {
  it.each(["claude", "codex", "opencode"])("takes %s", (name) => {
    expect(harnessUnderTest(name).kind).toBe(name);
  });

  it("types each harness's own syntax", () => {
    expect(harnessUnderTest("claude").typed("x")).toBe("/skills:x");
    expect(harnessUnderTest("codex").typed("x")).toBe("$x");
    expect(harnessUnderTest("opencode").typed("x")).toBe("/x");
  });

  it("refuses a run that names none, rather than checking nothing", () => {
    expect(() => harnessUnderTest(undefined)).toThrow(/PURLIS_OUTCOME_HARNESS/);
    expect(() => harnessUnderTest("cursor")).toThrow(/PURLIS_OUTCOME_HARNESS/);
  });
});
