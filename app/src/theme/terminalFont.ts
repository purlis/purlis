// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** What of a terminal this needs: xterm's live options. */
export type FontOptions = { options: { fontFamily?: string } };

/**
 * **Draws a terminal in `family`, measuring its cells again** (#2326).
 *
 * xterm measures a cell's width and height when its `fontFamily` option changes, and keeps
 * those numbers: a face that arrives later under the same family would be drawn on the old
 * fallback's grid, its glyphs clipped or spaced apart. Setting an option to the value it has
 * changes nothing, so when the family is the same and a face has just arrived (`arrived`), the
 * option is set to the bare generic family and back, which is xterm's own path to measure again.
 * Answers whether it changed what the terminal is drawn in; a refit follows either way.
 */
export function drawTerminalFont(pane: FontOptions, family: string, arrived: boolean): boolean {
  if (pane.options.fontFamily !== family) {
    pane.options.fontFamily = family;
    return true;
  }
  if (!arrived) return false;
  pane.options.fontFamily = "monospace";
  pane.options.fontFamily = family;
  return true;
}
