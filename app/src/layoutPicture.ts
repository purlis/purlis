// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { shareOf, WHOLE } from "./laidOut";
import { panes, type Layout } from "./tabs";

/** A pane's tile in the picture's box, which has side 1: its corner at (x, y), w wide, h tall. */
export type Tile = { x: number; y: number; w: number; h: number };

/** What a split tab's picture draws: how many panes it holds, and a tile for each. */
export type Picture = { panes: number; tiles: Tile[] };

/** The most panes a picture draws as they are laid out. Past it the shape is too small to read,
 *  and a grid says "many" instead. */
export const DRAWN_UP_TO = 4;

/** **Many**: a 3 × 3 grid, which no layout of four panes or fewer draws, so a tab of five is never
 *  read as a quad (spec #5339: "past four panes it's a grid"; #5577). How many is the hover's and
 *  the words'. */
const MANY: readonly Tile[] = [0, 1, 2].flatMap((row) =>
  [0, 1, 2].map((column) => ({ x: column / 3, y: row / 3, w: 1 / 3, h: 1 / 3 })),
);

/**
 * **A split tab's small picture of its layout's shape** (#5342, spec #5339), in place of `+1`: a
 * tile for each pane, drawn from the layout tree, each split at its share, so the picture is the
 * tab's own shape and its panes are counted at a glance. **Tiles, not lines between them**
 * (#5577): at a tab's size three panes drawn as lines read as the quad, a half line short; three
 * tiles never do. Past {@link DRAWN_UP_TO} panes it is a 3 × 3 grid. Nothing for a tab of one
 * pane, which has no picture.
 */
export function pictureOf(layout: Layout): Picture | undefined {
  const count = panes(layout).length;
  if (count < 2) return undefined;
  if (count > DRAWN_UP_TO) return { panes: count, tiles: [...MANY] };
  const tiles: Tile[] = [];
  const draw = (at: Layout, box: Tile) => {
    if (at.kind === "pane") {
      tiles.push(box);
      return;
    }
    const share = shareOf(at) / WHOLE;
    const [first, second] = at.children;
    if (at.direction === "row") {
      const w = box.w * share;
      draw(first, { ...box, w });
      draw(second, { ...box, x: box.x + w, w: box.w - w });
    } else {
      const h = box.h * share;
      draw(first, { ...box, h });
      draw(second, { ...box, y: box.y + h, h: box.h - h });
    }
  };
  draw(layout, { x: 0, y: 0, w: 1, h: 1 });
  return { panes: count, tiles };
}
