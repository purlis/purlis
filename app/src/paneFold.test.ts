// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { BAR, drawnOf, RAIL, splitRefused } from "./paneFold";
import { PANE_FLOOR } from "./paneDrop";
import type { Layout, Tabs } from "./tabs";
import { tileAll } from "./tiling";

/**
 * **A narrow window folds panes to rails** (#4766, spec #4750 stories 81 to 84): what is drawn
 * for a tab's tree in the room the window gives it, as plain values. A split that cannot give
 * each side its minimum folds the side without the focus to a rail (side by side) or a bar
 * (stacked); the tree itself is never changed.
 */

const pane = (n: number): Layout => ({
  kind: "pane",
  pane: n,
  content: { kind: "session", session: n, chat: `chat ${n}` },
});

const row = (a: Layout, b: Layout, share?: number): Layout => ({
  kind: "split",
  direction: "row",
  children: [a, b],
  ...(share === undefined ? {} : { share }),
});

const column = (a: Layout, b: Layout): Layout => ({
  kind: "split",
  direction: "column",
  children: [a, b],
});

/** A tab of nine panes, tiled 3 × 3 as Tile all tiles them. */
function grid(): Layout {
  const nine = [2, 3, 4, 5, 6, 7, 8, 9].reduce<Layout>((at, n) => row(at, pane(n)), pane(1));
  const tabs: Tabs = {
    byId: { 1: { id: 1, name: "chat 1", defaultName: "chat 1", layout: nine, focused: 1 } },
    order: [1],
    inFront: 1,
    named: { tabs: 1, panes: 9 },
  };
  return tileAll(tabs, 1).byId[1].layout;
}

/** The panes of each folded side, by the path of the split it folds in. */
const foldedIn = (layout: Layout, focused: number, width: number, height: number) =>
  Object.fromEntries(
    Object.entries(drawnOf(layout, focused, { width, height }).folds).map(([path, fold]) => [
      path,
      fold.panes,
    ]),
  );

describe("which side of a split folds (#4766)", () => {
  it("folds nothing while each side has its minimum", () => {
    const drawn = drawnOf(row(pane(1), pane(2)), 1, { width: 800, height: 600 });

    expect(drawn.folds).toEqual({});
    expect(drawn.sizes[1]).toEqual({ width: 400, height: 600 });
    expect(drawn.sizes[2]).toEqual({ width: 400, height: 600 });
  });

  it("folds the side without the focus to a rail when side by side, and gives the other the rest", () => {
    const drawn = drawnOf(row(pane(1), pane(2)), 2, { width: 500, height: 600 });

    expect(drawn.folds).toEqual({ "": { side: 0, panes: [1], as: "rail" } });
    expect(drawn.sizes[2]).toEqual({ width: 500 - RAIL, height: 600 });
    expect(drawn.sizes[1]).toBeUndefined();
  });

  it("folds to a bar when stacked", () => {
    const drawn = drawnOf(column(pane(1), pane(2)), 1, { width: 800, height: 200 });

    expect(drawn.folds).toEqual({ "": { side: 1, panes: [2], as: "bar" } });
    expect(drawn.sizes[1]).toEqual({ width: 800, height: 200 - BAR });
  });

  it("folds the second side when neither holds the focus", () => {
    expect(foldedIn(row(pane(1), pane(2)), 7, 500, 600)).toEqual({ "": [2] });
  });

  it("folds one nesting level at a time: a whole unfocused side is one rail", () => {
    // 1 | (2 | 3), focus on 3: the outer split needs 900px and has 700, so 1 folds; the inner
    // one then has 660, which holds both 2 and 3.
    expect(foldedIn(row(pane(1), row(pane(2), pane(3))), 3, 700, 600)).toEqual({ "": [1] });
    // Focus on 1: the side holding 2 and 3 folds whole, into one rail.
    expect(foldedIn(row(pane(1), row(pane(2), pane(3))), 1, 700, 600)).toEqual({ "": [2, 3] });
  });

  it("keeps a split's shares where they fit, within each side's minimum", () => {
    const drawn = drawnOf(row(pane(1), pane(2), 900), 1, { width: 1000, height: 600 });

    expect(drawn.sizes[1].width).toBe(1000 - PANE_FLOOR.width);
    expect(drawn.sizes[2].width).toBe(PANE_FLOOR.width);
  });

  it("never changes the tree it is drawn from", () => {
    const layout = grid();
    const before = structuredClone(layout);

    drawnOf(layout, 5, { width: 400, height: 200 });

    expect(layout).toEqual(before);
  });

  it("brings the layout back when there is room again", () => {
    const layout = row(pane(1), pane(2), 600);

    expect(drawnOf(layout, 1, { width: 500, height: 600 }).folds).not.toEqual({});
    const wide = drawnOf(layout, 1, { width: 1000, height: 600 });
    expect(wide.folds).toEqual({});
    expect(wide.sizes[1].width).toBe(600);
  });
});

describe("a 3 × 3 grid in a narrow window (#4766)", () => {
  it("at 770px keeps one column per row and rails the rest", () => {
    // Tile all: three rows stacked, each a | (b | c). Each row needs 900px across.
    const folds = foldedIn(grid(), 1, 770, 600);

    expect(folds).toEqual({ a: [2, 3], ba: [5, 6], bb: [8, 9] });
    const sizes = drawnOf(grid(), 1, { width: 770, height: 600 }).sizes;
    expect(Object.keys(sizes).map(Number)).toEqual([1, 4, 7]);
  });

  it("folds rows into bars too when the window is short as well", () => {
    // Three rows need 420px down; at 300 the two rows without the focus fold into one bar.
    const folds = drawnOf(grid(), 1, { width: 770, height: 300 }).folds;

    expect(folds[""]).toEqual({ side: 1, panes: [4, 5, 6, 7, 8, 9], as: "bar" });
  });
});

describe("each split's minimum is its subtree's (#4766)", () => {
  it("sums along its axis and takes the largest across it", () => {
    // (1 / 2) | 3: the left side needs 300 across and 280 down; the split 600 across.
    const layout = row(column(pane(1), pane(2)), pane(3));

    expect(foldedIn(layout, 3, 600, 600)).toEqual({});
    expect(foldedIn(layout, 3, 599, 600)).toEqual({ "": [1, 2] });
    // At 279 down the left side cannot stack its two panes, and folds one of them to a bar.
    expect(foldedIn(layout, 1, 600, 279)).toEqual({ a: [2] });
  });
});

describe("Split right and Split down below the minimum (#4766)", () => {
  it("are refused, with the reason, where a half would be under the minimum", () => {
    const narrow = { width: 2 * PANE_FLOOR.width - 1, height: 600 };
    const short = { width: 900, height: 2 * PANE_FLOOR.height - 1 };

    expect(splitRefused(narrow, "row")).toBe(
      "Too narrow to split: each side would be under about 40 columns. Widen the window or close a pane.",
    );
    expect(splitRefused(narrow, "column")).toBeUndefined();
    expect(splitRefused(short, "column")).toBe(
      "Too short to split: each side would be under about 8 rows. Make the window taller or close a pane.",
    );
    expect(splitRefused(short, "row")).toBeUndefined();
  });

  it("are allowed at exactly twice the minimum, and where the size is not known", () => {
    expect(
      splitRefused({ width: 2 * PANE_FLOOR.width, height: 2 * PANE_FLOOR.height }, "row"),
    ).toBeUndefined();
    expect(splitRefused(undefined, "row")).toBeUndefined();
  });
});
