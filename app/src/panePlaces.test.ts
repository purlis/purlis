// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { paneNamesOf, paneTakingTheSpace, paneToward } from "./panePlaces";
import type { Layout } from "./tabs";

/**
 * **Where each pane is in its tab** (#4767, ruled on #4699): the pane ⌘⌥ and an arrow moves to,
 * the name a screen reader hears for each pane, and the pane that takes a closed one's space.
 * All of it worked out from the tab's tree alone.
 */

const pane = (n: number): Layout => ({
  kind: "pane",
  pane: n,
  content: { kind: "empty", workspace: "ops" },
});
const row = (a: Layout, b: Layout, share?: number): Layout => ({
  kind: "split",
  direction: "row",
  children: [a, b],
  ...(share === undefined ? {} : { share }),
});
const column = (a: Layout, b: Layout, share?: number): Layout => ({
  kind: "split",
  direction: "column",
  children: [a, b],
  ...(share === undefined ? {} : { share }),
});

/**
 * ┌───┬───┐
 * │ 1 │ 2 │
 * ├───┼───┤
 * │ 3 │ 4 │
 * └───┴───┘
 */
const grid = row(column(pane(1), pane(3)), column(pane(2), pane(4)));

/**
 * ┌───────┐
 * │   1   │
 * ├───┬───┤
 * │ 2 │ 3 │
 * └───┴───┘
 */
const topAndTwo = column(pane(1), row(pane(2), pane(3)));

describe("the pane in a direction (⌘⌥ and an arrow)", () => {
  it("moves to the pane beside, above and below", () => {
    expect(paneToward(grid, 1, "right")).toBe(2);
    expect(paneToward(grid, 2, "left")).toBe(1);
    expect(paneToward(grid, 1, "down")).toBe(3);
    expect(paneToward(grid, 4, "up")).toBe(2);
  });

  it("does not wrap: at the edge nothing moves", () => {
    expect(paneToward(grid, 1, "left")).toBeUndefined();
    expect(paneToward(grid, 1, "up")).toBeUndefined();
    expect(paneToward(grid, 4, "right")).toBeUndefined();
    expect(paneToward(pane(1), 1, "right")).toBeUndefined();
  });

  it("from a wide pane, goes to the pane under most of it, the first one on a tie", () => {
    expect(paneToward(topAndTwo, 1, "down")).toBe(2);
    expect(paneToward(column(pane(1), row(pane(2), pane(3), 300)), 1, "down")).toBe(3);
    expect(paneToward(topAndTwo, 3, "up")).toBe(1);
  });

  it("reads where each pane is from the split's share, not from the tree's shape", () => {
    // ┌──────┬─┐   1 is above 3 and spans most of the width, 2 is a sliver on the right;
    // │  1   │2│   below, 3 and 4 split evenly, so 4 starts left of 2's edge.
    // ├────┬─┴─┤
    // │ 3  │ 4 │
    // └────┴───┘
    const uneven = column(row(pane(1), pane(2), 900), row(pane(3), pane(4)));
    expect(paneToward(uneven, 2, "down")).toBe(4);
    expect(paneToward(uneven, 4, "up")).toBe(1);
  });

  it("knows nothing of a pane that is not in the tab", () => {
    expect(paneToward(grid, 9, "right")).toBeUndefined();
  });
});

describe("panes are named by where they are", () => {
  it("names a pane alone in its tab plainly", () => {
    expect(paneNamesOf(pane(1)).get(1)).toBe("pane");
  });

  it("names two side by side left and right, two stacked top and bottom", () => {
    expect([...paneNamesOf(row(pane(1), pane(2))).values()]).toEqual(["left pane", "right pane"]);
    expect([...paneNamesOf(column(pane(1), pane(2))).values()]).toEqual([
      "top pane",
      "bottom pane",
    ]);
  });

  it("names a grid's corners, and a pane spanning the width by its side alone", () => {
    expect(Object.fromEntries(paneNamesOf(grid))).toEqual({
      1: "top left pane",
      2: "top right pane",
      3: "bottom left pane",
      4: "bottom right pane",
    });
    expect(Object.fromEntries(paneNamesOf(topAndTwo))).toEqual({
      1: "top pane",
      2: "bottom left pane",
      3: "bottom right pane",
    });
  });

  it("names a pane between two others the middle one", () => {
    expect([...paneNamesOf(row(pane(1), row(pane(2), pane(3)))).values()]).toEqual([
      "left pane",
      "middle pane",
      "right pane",
    ]);
  });

  it("numbers panes that would share a name, in reading order", () => {
    const four = row(pane(1), row(row(pane(2), pane(3)), pane(4)));
    expect([...paneNamesOf(four).values()]).toEqual([
      "left pane",
      "middle pane 1",
      "middle pane 2",
      "right pane",
    ]);
  });
});

describe("the pane that takes a closed one's space", () => {
  it("is the pane beside it across the split it leaves", () => {
    expect(paneTakingTheSpace(row(pane(1), pane(2)), 1)).toBe(2);
    expect(paneTakingTheSpace(row(pane(1), pane(2)), 2)).toBe(1);
  });

  it("is the nearest pane of the half that grows, not the tab's first pane", () => {
    // Closing 4 grows 2 into its space: 2 is the pane that was beside it, though 1 comes first.
    expect(paneTakingTheSpace(grid, 4)).toBe(2);
    // Closing 1, the whole bottom row grows upwards; 2 is the first pane under it.
    expect(paneTakingTheSpace(topAndTwo, 1)).toBe(2);
  });

  it("is nothing for the only pane, or one not in the tab", () => {
    expect(paneTakingTheSpace(pane(1), 1)).toBeUndefined();
    expect(paneTakingTheSpace(grid, 9)).toBeUndefined();
  });
});
