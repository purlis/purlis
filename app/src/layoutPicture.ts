// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { shareOf, WHOLE } from "./laidOut";
import { panes, type Layout } from "./tabs";

/** A line that divides the picture's box, which has side 1, from (x1, y1) to (x2, y2). */
export type Divider = { x1: number; y1: number; x2: number; y2: number };

/** What a split tab's picture draws: how many panes it holds, and the lines between them. */
export type Picture = { panes: number; lines: Divider[] };

/** The most panes a picture draws as they are laid out. Past it the shape is too small to read,
 *  and a grid says "many" instead. */
export const DRAWN_UP_TO = 4;

/** A quad, as the prototype draws every tab of four panes or more (spec #5339: "past four
 *  panes it's a grid, as the prototype does"): how many is the hover's and the words'. */
const GRID: readonly Divider[] = [
  { x1: 0.5, y1: 0, x2: 0.5, y2: 1 },
  { x1: 0, y1: 0.5, x2: 1, y2: 0.5 },
];

type Box = { x: number; y: number; w: number; h: number };

/**
 * **A split tab's small picture of its layout's shape** (#5342, spec #5339), in place of `+1`:
 * the lines between its panes, drawn from the layout tree, each split at its share, so the
 * picture is the tab's own shape. Past {@link DRAWN_UP_TO} panes it is a grid. Nothing for a tab
 * of one pane, which has no picture.
 */
export function pictureOf(layout: Layout): Picture | undefined {
  const count = panes(layout).length;
  if (count < 2) return undefined;
  if (count > DRAWN_UP_TO) return { panes: count, lines: [...GRID] };
  const lines: Divider[] = [];
  const draw = (at: Layout, box: Box) => {
    if (at.kind === "pane") return;
    const share = shareOf(at) / WHOLE;
    const [first, second] = at.children;
    if (at.direction === "row") {
      const x = box.x + box.w * share;
      lines.push({ x1: x, y1: box.y, x2: x, y2: box.y + box.h });
      draw(first, { ...box, w: x - box.x });
      draw(second, { ...box, x, w: box.x + box.w - x });
    } else {
      const y = box.y + box.h * share;
      lines.push({ x1: box.x, y1: y, x2: box.x + box.w, y2: y });
      draw(first, { ...box, h: y - box.y });
      draw(second, { ...box, y, h: box.y + box.h - y });
    }
  };
  draw(layout, { x: 0, y: 0, w: 1, h: 1 });
  return { panes: count, lines };
}
