// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Whether `element`'s words are drawn cut**: wider or taller than its box shows. A table's cell
 * cut to its column (`CellWords`) and a list's line clamped under a row (`RowBelow`, #4711) both
 * say their whole words in a tooltip only while this holds. Half a pixel is a rounding, not a cut.
 */
export function drawnCut(element: HTMLElement): boolean {
  return (
    element.scrollWidth > element.clientWidth + 0.5 ||
    element.scrollHeight > element.clientHeight + 0.5
  );
}
