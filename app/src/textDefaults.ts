// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The two text sizes, before any machine's**: which they are, what a machine starts at, and
 * how the window's is drawn. `textSize.ts` reads and keeps a machine's own; this module imports
 * nothing, so a page that draws only the defaults (the theme specimen, #2330) reaches
 * nothing of the person's through it.
 */

/** Which of the two sizes. */
export type Which = "window" | "terminal";

export type TextSizes = Record<Which, number>;

/**
 * The sizes a machine starts at: **one step bigger than both were** (#283). The window was
 * `13px` in `App.css` and the terminal `12` in `SessionPane`.
 */
export const DEFAULT_TEXT: TextSizes = { window: 14, terminal: 13 };

/** Draws the window's text at `px`: the root's font size, which every `rem` is measured by. */
export function drawWindowText(px: number, root: HTMLElement = document.documentElement): void {
  root.style.fontSize = `${px}px`;
}
