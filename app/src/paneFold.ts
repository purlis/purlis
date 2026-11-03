// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { shareOf, WHOLE } from "./laidOut";
import { floorAlong, PANE_FLOOR, type Size } from "./paneDrop";
import { panes, type Layout } from "./tabs";

/**
 * **A narrow window folds panes to rails** (#4766, spec #4750 stories 81 to 84;
 * `prototype/narrow-window` Q3 A and Q4 "panes fold, sides stay", accepted on #4702).
 *
 * - **A split that can't give both sides their minimum folds the side without the focus**: to a
 *   {@link RAIL} beside the other when side by side, to a {@link BAR} above or below it when
 *   stacked. The side holding the focus takes the rest of the room.
 * - **One nesting level at a time**: the outermost split that does not fit folds first, and its
 *   unfocused side folds whole, into one rail listing every pane in it. The kept side is then
 *   laid out in what is left, and may fold again.
 * - **Each split's minimum is its subtree's** ({@link floorAlong}): the sum along its axis and
 *   the largest across it.
 * - **Folding is decided when drawing, and never written to the tree**, so the layout comes back
 *   with its shares once there is room. The sidebar and the Explorer stay as they were: only the
 *   room the panes are given is read here.
 *
 * Separators are not counted, as a drop's refusal does not count them (`judgeDrop`): the
 * minimum is about 40 × 8, and a separator is a few pixels of it.
 */

/** How wide a folded side is beside its sibling, in CSS pixels. */
export const RAIL = 40;

/** How tall a folded side is above or below its sibling, in CSS pixels. */
export const BAR = 32;

/** One folded side of a split: which side, the panes in it in reading order, and its shape. */
export type Folded = { side: 0 | 1; panes: number[]; as: "rail" | "bar" };

/**
 * **What a tab's tree draws in the room it has**: the folded side of each split that folds, by
 * the split's path from the tab's root (`a` for a split's first child, `b` for its second, as
 * `LayoutPanes` names them), and the size of each pane drawn whole. A folded pane has no size.
 */
export type Drawn = { folds: Record<string, Folded>; sizes: Record<number, Size> };

/** **What `layout` draws in `room`, with pane `focused` holding the focus.** */
export function drawnOf(layout: Layout, focused: number, room: Size): Drawn {
  const drawn: Drawn = { folds: {}, sizes: {} };
  lay(layout, focused, room, "", drawn);
  return drawn;
}

function lay(layout: Layout, focused: number, room: Size, path: string, drawn: Drawn): void {
  if (layout.kind === "pane") {
    drawn.sizes[layout.pane] = room;
    return;
  }
  const across = layout.direction === "row";
  const along = across ? room.width : room.height;
  const [a, b] = layout.children;
  const least = [floorAlong(a, layout.direction), floorAlong(b, layout.direction)];
  const sized = (length: number): Size =>
    across ? { width: length, height: room.height } : { width: room.width, height: length };

  if (along >= least[0] + least[1]) {
    // Each side at its share, kept within both sides' minimum, as the separator keeps it.
    const wanted = Math.round((along * shareOf(layout)) / WHOLE);
    const first = Math.min(Math.max(wanted, least[0]), along - least[1]);
    lay(a, focused, sized(first), `${path}a`, drawn);
    lay(b, focused, sized(along - first), `${path}b`, drawn);
    return;
  }
  // The side holding the focus keeps the room; with the focus in neither, the first does.
  const kept = holds(b, focused) && !holds(a, focused) ? 1 : 0;
  const side = kept === 0 ? 1 : 0;
  const shape = across ? "rail" : "bar";
  drawn.folds[path] = {
    side,
    panes: panes(layout.children[side]).map(({ pane }) => pane),
    as: shape,
  };
  lay(
    layout.children[kept],
    focused,
    sized(along - (across ? RAIL : BAR)),
    `${path}${kept === 0 ? "a" : "b"}`,
    drawn,
  );
}

const holds = (layout: Layout, pane: number): boolean =>
  panes(layout).some((one) => one.pane === pane);

/**
 * **Why Split right (`row`) or Split down (`column`) is refused for a pane of `size`**, or
 * nothing when it can split: each half must keep the minimum (`PANE_FLOOR`). A split greys out
 * with this reason; it never switches direction by itself (spec #4750 story 84). A pane whose
 * size is not known yet can split, as before it was measured.
 */
export function splitRefused(
  size: Size | undefined,
  direction: "row" | "column",
): string | undefined {
  if (size === undefined) return undefined;
  if (direction === "row")
    return size.width < 2 * PANE_FLOOR.width
      ? "Too narrow to split: each side would be under about 40 columns. Widen the window or close a pane."
      : undefined;
  return size.height < 2 * PANE_FLOOR.height
    ? "Too short to split: each side would be under about 8 rows. Make the window taller or close a pane."
    : undefined;
}

/** The splits a pane's head and the palette offer, by their catalogue row. */
const SPLITS: Readonly<Record<string, "row" | "column">> = {
  "pane.split.right": "row",
  "pane.split.down": "column",
};

/**
 * **`offer` for a pane of `size`**: a split row greyed with {@link splitRefused}'s reason where a
 * half would be under the minimum, and any other row, or a split that fits, as it is.
 */
export function refusedAt<T extends { id: string; available: boolean; reason: string }>(
  offer: T | undefined,
  size: Size | undefined,
): T | undefined {
  const direction = offer === undefined ? undefined : SPLITS[offer.id];
  if (offer === undefined || direction === undefined || !offer.available) return offer;
  const why = splitRefused(size, direction);
  return why === undefined ? offer : { ...offer, available: false, reason: why };
}
