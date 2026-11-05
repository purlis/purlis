// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

/** True from `ms` from now on, and until then what a wait keeps trying. */
const trueAfter = (ms: number) => {
  const from = performance.now() + ms;
  return () => performance.now() >= from;
};

const never = () => {
  throw new Error("not yet");
};

const WELL_BELOW_THE_BUDGET = 4_000;

/** How long `wait` took to give up. */
const givesUpAfter = async (wait: () => Promise<unknown>) => {
  const from = performance.now();
  await expect(wait()).rejects.toThrow();
  return performance.now() - from;
};

// #5249: Vitest's own waits stop at a second unless they are told otherwise, and no setting of
// Vitest's reaches `vi.waitFor`. A wait that stopped at a second took a merge group down when a
// loaded runner drew the window's first frame at 1.2 s.
describe("how long a test's waits last (#5249)", () => {
  it("gives every kind of wait longer than Vitest's second", async () => {
    const soon = trueAfter(1_200);
    const held = () => {
      if (!soon()) never();
    };

    await Promise.all([
      vi.waitFor(held),
      vi.waitFor(held, { interval: 10 }),
      vi.waitFor(held, { timeout: undefined }),
      vi.waitUntil(soon),
      expect.poll(soon).toBe(true),
      waitFor(held),
    ]);
  });

  it("keeps the timeout a wait says for itself, in either spelling", async () => {
    // A wait that had lost its own 100 ms would take the whole budget to give up; one that kept
    // it is well clear of that however loaded the runner is.
    expect(await givesUpAfter(() => vi.waitFor(never, { timeout: 100 }))).toBeLessThan(
      WELL_BELOW_THE_BUDGET,
    );
    expect(await givesUpAfter(() => vi.waitFor(never, 100))).toBeLessThan(WELL_BELOW_THE_BUDGET);
    expect(await givesUpAfter(() => vi.waitUntil(() => false, { timeout: 100 }))).toBeLessThan(
      WELL_BELOW_THE_BUDGET,
    );
    expect(
      await givesUpAfter(() => expect.poll(() => false, { timeout: 100 }).toBe(true)),
    ).toBeLessThan(WELL_BELOW_THE_BUDGET);
  });
});
