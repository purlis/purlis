// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { pictureOf, type Divider } from "./layoutPicture";
import type { Layout } from "./tabs";

/**
 * **A split tab's small picture of its layout** (#5342, spec #5339): the lines that divide its
 * box, drawn from the tab's layout tree in a box of side 1, each split at its share. Past four
 * panes it is a grid, which no layout of four or fewer draws. A tab of one pane has no picture.
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
/** A line from (x1, y1) to (x2, y2). */
const line = (x1: number, y1: number, x2: number, y2: number): Divider => ({ x1, y1, x2, y2 });
const GRID = [
  line(1 / 3, 0, 1 / 3, 1),
  line(2 / 3, 0, 2 / 3, 1),
  line(0, 1 / 3, 1, 1 / 3),
  line(0, 2 / 3, 1, 2 / 3),
];

const table: readonly [string, Layout, { panes: number; lines: Divider[] } | undefined][] = [
  ["one pane has no picture", chat(), undefined],
  ["two side by side: a line down the middle", row(chat(), chat()), { panes: 2, lines: [line(0.5, 0, 0.5, 1)] }],
  ["two one over the other: a line across", column(chat(), chat()), { panes: 2, lines: [line(0, 0.5, 1, 0.5)] }],
  ["a split moved to 30%: its line at 30%", row(chat(), chat(), 300), { panes: 2, lines: [line(0.3, 0, 0.3, 1)] }],
  [
    "three, the right half split again: the second line on the right only",
    row(chat(), column(chat(), chat())),
    { panes: 3, lines: [line(0.5, 0, 0.5, 1), line(0.5, 0.5, 1, 0.5)] },
  ],
  [
    "three in a row: two lines, the second in the right half",
    row(chat(), row(chat(), chat())),
    { panes: 3, lines: [line(0.5, 0, 0.5, 1), line(0.75, 0, 0.75, 1)] },
  ],
  [
    "four in a quad",
    row(column(chat(), chat()), column(chat(), chat())),
    {
      panes: 4,
      lines: [line(0.5, 0, 0.5, 1), line(0, 0.5, 0.5, 0.5), line(0.5, 0.5, 1, 0.5)],
    },
  ],
  [
    "five: a grid, whatever their shape",
    row(chat(), row(chat(), row(chat(), row(chat(), chat())))),
    { panes: 5, lines: GRID },
  ],
];

describe("a split tab's picture of its layout", () => {
  it.each(table)("%s", (_, layout, picture) => {
    expect(pictureOf(layout)).toEqual(picture);
  });
});
