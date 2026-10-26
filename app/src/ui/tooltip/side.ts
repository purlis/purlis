// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Which side a tooltip stands on** (#3328, R166). A tooltip stands at its target's end, and is
 * flipped to its start where the end has no room. React Aria flips it only to the other side, and
 * a tooltip set by its end against the window's left edge takes only the room left of its target:
 * against a row that runs across the window it came out a column of single words. So the side is
 * chosen here, for the tooltip's whole width:
 *
 * - **the end**, where it has room for the whole tooltip;
 * - else **the start**, where that has room (unless the end was taken, `preferred` "start");
 * - else **under the target** (React Aria flips that over it at the window's foot), where a
 *   tooltip has the window's width.
 */

/** A side a tooltip can stand on. */
export type TooltipSide = "end" | "start" | "bottom";

/** How far a tooltip keeps from the window's edge, in px: React Aria's container padding. */
export const TOOLTIP_EDGE = 12;

/**
 * The side a tooltip `width` px wide stands on, `gap` px from `target` (its box in the window's
 * coordinates), in a window `view` px wide. `preferred` "start" says the end is taken.
 */
export function sideWithRoom(
  preferred: "end" | "start",
  target: { left: number; right: number },
  width: number,
  gap: number,
  view: number,
  rtl: boolean,
): TooltipSide {
  const need = width + gap + TOOLTIP_EDGE;
  const right = view - target.right;
  const left = target.left;
  const atEnd = rtl ? left : right;
  const atStart = rtl ? right : left;
  if (preferred === "end" && atEnd >= need) return "end";
  if (atStart >= need) return "start";
  return "bottom";
}
