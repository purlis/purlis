// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { askedIn, handOf } from "./askedIn";
import { dockTotal } from "./dockBadge";

/**
 * **The ✋ counts the project in front, each project tab its own, the Dock every one** (#6341,
 * ADR 0121 amended 2026-11-12, decision 1): one count per open project of every window, which
 * the ✋ reads for the project in front and the Dock badge adds up.
 */

const ONE = "/home/dev/one";
const TWO = "/home/dev/two";
const FAR = "/home/dev/far";
const UNTOLD = "/home/dev/untold";

/** ONE and TWO in this window, FAR in another that told its asks, UNTOLD in one that did not. */
const asked = askedIn({
  planes: [ONE, TWO],
  held: { [ONE]: ["a", "b"], [TWO]: ["c"] },
  needing: [],
  there: [{ plane: FAR, count: 3 }],
  asking: [{ plane: UNTOLD }, { plane: UNTOLD }, { plane: FAR }],
});

describe("the ✋ and the Dock badge (#6341)", () => {
  it("counts only the project in front on the ✋", () => {
    expect(handOf(asked, ONE)).toBe(2);
    expect(handOf(asked, TWO)).toBe(1);
  });

  it("counts every open project of every window on the Dock", () => {
    expect(dockTotal(asked)).toBe(2 + 1 + 3 + 2);
  });

  it("gives each project its own count, which add up to the Dock's", () => {
    expect(Object.fromEntries(asked)).toEqual({ [ONE]: 2, [TWO]: 1, [FAR]: 3, [UNTOLD]: 2 });
  });

  it("counts nothing on the ✋ with no project in front", () => {
    expect(handOf(asked, undefined)).toBe(0);
  });

  it("counts a project's chats where its registry has said nothing yet", () => {
    const early = askedIn({
      planes: [ONE],
      held: {},
      needing: [{ plane: ONE }, { plane: ONE }],
      there: [],
      asking: [],
    });
    expect(handOf(early, ONE)).toBe(2);
  });

  it("never counts a project of this window again that another window still tells", () => {
    const moved = askedIn({
      planes: [ONE],
      held: { [ONE]: ["a"] },
      needing: [],
      there: [{ plane: ONE, count: 1 }],
      asking: [{ plane: ONE }],
    });
    expect(dockTotal(moved)).toBe(1);
  });
});
