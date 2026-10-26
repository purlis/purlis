// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Which side a tooltip stands on** (#3328, R166): its target's end, flipped to its start where
 * the end has no room for the whole tooltip, and under (or over) it where neither side has.
 */
import { describe, expect, it } from "vitest";
import { sideWithRoom, TOOLTIP_EDGE } from "./side";
import { TOOLTIP_OFFSET } from "./Tooltip";

/** A window 1200 px wide, and a tooltip 300 px wide. */
const VIEW = 1200;
const WIDE = 300;
const NEED = WIDE + TOOLTIP_OFFSET + TOOLTIP_EDGE;

describe("a tooltip's side", () => {
  it("stands at its target's end where the end has room", () => {
    expect(sideWithRoom("end", { left: 100, right: 400 }, WIDE, TOOLTIP_OFFSET, VIEW, false)).toBe(
      "end",
    );
  });

  it("flips to the start where only the start has room", () => {
    expect(sideWithRoom("end", { left: 600, right: 1100 }, WIDE, TOOLTIP_OFFSET, VIEW, false)).toBe(
      "start",
    );
  });

  it("stands under a full-width row, which has room on neither side (the bug in #3328)", () => {
    // The left panel closed: a bottom panel's row runs from the rail to the window's edge.
    expect(sideWithRoom("end", { left: 110, right: VIEW }, WIDE, TOOLTIP_OFFSET, VIEW, false)).toBe(
      "bottom",
    );
  });

  it("needs room for the whole tooltip, its gap and the window's edge", () => {
    expect(
      sideWithRoom("end", { left: 0, right: VIEW - NEED }, WIDE, TOOLTIP_OFFSET, VIEW, false),
    ).toBe("end");
    expect(
      sideWithRoom("end", { left: 0, right: VIEW - NEED + 1 }, WIDE, TOOLTIP_OFFSET, VIEW, false),
    ).toBe("bottom");
  });

  it("reads the end as the left in a right-to-left window", () => {
    expect(sideWithRoom("end", { left: 600, right: 900 }, WIDE, TOOLTIP_OFFSET, VIEW, true)).toBe(
      "end",
    );
    expect(sideWithRoom("end", { left: 100, right: 400 }, WIDE, TOOLTIP_OFFSET, VIEW, true)).toBe(
      "start",
    );
  });

  it("keeps off the end when the start was asked for, as the end is taken (a submenu)", () => {
    expect(
      sideWithRoom("start", { left: 600, right: 700 }, WIDE, TOOLTIP_OFFSET, VIEW, false),
    ).toBe("start");
    expect(
      sideWithRoom("start", { left: 100, right: 200 }, WIDE, TOOLTIP_OFFSET, VIEW, false),
    ).toBe("bottom");
  });
});
