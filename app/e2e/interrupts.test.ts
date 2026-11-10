// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { CEILINGS, DEFAULT_CEILING, ceilingOf, overCeiling, overSaid } from "./interrupts.js";

describe("a flow's interruptions (#6021)", () => {
  it("are within the ceiling at it, and over it past it", () => {
    expect(overCeiling({ prompts: 3, asks: 3, notifications: 1 }, DEFAULT_CEILING)).toEqual([]);
    expect(overCeiling({ prompts: 4, asks: 0, notifications: 2 }, DEFAULT_CEILING)).toEqual([
      "4 prompts (ceiling 3)",
      "2 notifications (ceiling 1)",
    ]);
  });

  it("are held to W10's three prompts where the file names no ceiling", () => {
    expect(ceilingOf("/run/app/e2e/specs/unnamed.e2e.ts")).toEqual(DEFAULT_CEILING);
    expect(DEFAULT_CEILING.prompts).toBe(3);
  });

  it("are held to a named file's ceiling only with its reason", () => {
    for (const [file, named] of Object.entries(CEILINGS)) {
      expect(named.why.length, file).toBeGreaterThan(20);
      expect(ceilingOf(`/x/specs/${file}`)).toEqual({ ...DEFAULT_CEILING, ...named.ceiling });
    }
  });

  it("are said naming the file, each flow over and what it cost", () => {
    expect(
      overSaid("/run/app/e2e/specs/opener.e2e.ts", [
        { flow: "opening a stranger's project asks", past: ["4 prompts (ceiling 3)"] },
      ]),
    ).toBe(
      'specs/opener.e2e.ts: a flow interrupts more than its ceiling allows: "opening a ' +
        "stranger's project asks\": 4 prompts (ceiling 3). Take the interruption out, or raise " +
        "the file's ceiling in e2e/interrupts.ts with the reason (#6021).",
    );
  });
});
