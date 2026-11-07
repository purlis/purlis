// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { pictureOf, type Picture, type Tile } from "./layoutPicture";
import type { Layout } from "./tabs";

/**
 * **A split tab's small picture of its layout** (#5342, spec #5339): a tile for each pane, drawn
 * from the tab's layout tree in a box of side 1, each split at its share, so three panes are three
 * tiles and never read as a quad at a tab's size (#5577). Past four panes it is a 3 × 3 grid. A tab
 * of one pane has no picture.
 */
let next = 0;
const chat = (): Layout => ({
  kind: "pane",
  pane: ++next,
  content: { kind: "session", session: next, chat: String(next) },
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
/** A pane's tile at (x, y), w wide and h tall. */
const tile = (x: number, y: number, w: number, h: number): Tile => ({ x, y, w, h });
/** Past four panes: a 3 × 3 grid, which no layout of four or fewer draws. */
const MANY = [0, 1, 2].flatMap((row) =>
  [0, 1, 2].map((column) => tile(column / 3, row / 3, 1 / 3, 1 / 3)),
);

const table: readonly [string, Layout, Picture | undefined][] = [
  ["one pane has no picture", chat(), undefined],
  [
    "two side by side: two tiles, each a half",
    row(chat(), chat()),
    { panes: 2, tiles: [tile(0, 0, 0.5, 1), tile(0.5, 0, 0.5, 1)] },
  ],
  [
    "two one over the other: two tiles, one on the other",
    column(chat(), chat()),
    { panes: 2, tiles: [tile(0, 0, 1, 0.5), tile(0, 0.5, 1, 0.5)] },
  ],
  [
    "a split moved to 30%: the first tile 30% wide",
    row(chat(), chat(), 300),
    { panes: 2, tiles: [tile(0, 0, 0.3, 1), tile(0.3, 0, 0.7, 1)] },
  ],
  [
    "three, the right half split again: a tall tile and two small ones",
    row(chat(), column(chat(), chat())),
    { panes: 3, tiles: [tile(0, 0, 0.5, 1), tile(0.5, 0, 0.5, 0.5), tile(0.5, 0.5, 0.5, 0.5)] },
  ],
  [
    "three in a row: three tiles, the second split off the right half",
    row(chat(), row(chat(), chat())),
    { panes: 3, tiles: [tile(0, 0, 0.5, 1), tile(0.5, 0, 0.25, 1), tile(0.75, 0, 0.25, 1)] },
  ],
  [
    "four in a quad: four small tiles",
    row(column(chat(), chat()), column(chat(), chat())),
    {
      panes: 4,
      tiles: [
        tile(0, 0, 0.5, 0.5),
        tile(0, 0.5, 0.5, 0.5),
        tile(0.5, 0, 0.5, 0.5),
        tile(0.5, 0.5, 0.5, 0.5),
      ],
    },
  ],
  [
    "five: the 3 × 3 grid, whatever their shape",
    row(chat(), row(chat(), row(chat(), row(chat(), chat())))),
    { panes: 5, tiles: MANY },
  ],
];

describe("a split tab's picture of its layout", () => {
  it.each(table)("%s", (_, layout, picture) => {
    const drawn = pictureOf(layout);
    // Each to its own few places: a share is a fraction of a thousand.
    const round = (n: number) => Math.round(n * 1000) / 1000;
    const rounded = (one: Picture | undefined) =>
      one && {
        ...one,
        tiles: one.tiles.map((t) => tile(round(t.x), round(t.y), round(t.w), round(t.h))),
      };
    expect(rounded(drawn)).toEqual(rounded(picture));
  });
});
