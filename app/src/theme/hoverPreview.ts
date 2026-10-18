// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { drawIn, inForce, onDrawn, type Theme } from "./theme";

/**
 * **A theme drawn live while the pointer or the keyboard is on its card** (Settings › Theme,
 * #2331, spec #2313), and the person's own theme put back when it leaves.
 *
 * The window only applies: the card's theme is the core's answer (`theme_card_drawn`), drawn
 * with {@link drawIn} like any theme, so the stylesheet, every terminal and the fonts follow it.
 * Nothing is written, and nothing about it outlives the card.
 *
 * **What goes back is what is in force underneath**, not what was drawn when the card was
 * entered: a theme the window draws while a card is shown (a workspace switch, a folder saved)
 * becomes what goes back, and the card stays drawn over it until it is left.
 *
 * **Moving from one card to the next is one change, not two**: leaving puts the person's theme
 * back only after the events of the move are over, and the next card's theme replaces that
 * wait, so the window never flashes its own theme between two cards.
 */

/** The card's theme being drawn, while one is. */
let shown: Theme | null = null;
/** What is in force underneath it. */
let back: Theme | null = null;
/** The put-back waiting for the move's events to end. */
let leaving: ReturnType<typeof setTimeout> | undefined;
/** How to stop hearing the window's own draws, while a card is shown. */
let unfollow: (() => void) | undefined;

/** Draws `theme` while its card is pointed at or focused. */
export function showWhileOn(theme: Theme): void {
  if (leaving !== undefined) {
    clearTimeout(leaving);
    leaving = undefined;
  }
  if (shown === null) {
    back = inForce();
    unfollow = onDrawn((drawn) => {
      // The window drew a theme of its own under the card: that is what goes back.
      if (shown === null || drawn === shown) return;
      back = drawn;
      // After that draw has reached every follower, so none is left on the window's own.
      queueMicrotask(() => {
        if (shown !== null && inForce() !== shown) drawIn(shown);
      });
    });
  }
  shown = theme;
  if (inForce() !== theme) drawIn(theme);
}

/** Puts the person's theme back once the events of the move are over: unless another card is
 *  shown first. */
export function putBackSoon(): void {
  if (shown === null || leaving !== undefined) return;
  leaving = setTimeout(putBack, 0);
}

/** Puts the person's theme back at once: the card was left for good (the tab went, a choice
 *  was made). */
export function putBack(): void {
  if (leaving !== undefined) {
    clearTimeout(leaving);
    leaving = undefined;
  }
  if (shown === null) return;
  const to = back;
  shown = null;
  back = null;
  unfollow?.();
  unfollow = undefined;
  if (to !== null && to !== inForce()) drawIn(to);
}

/** Whether a card's theme is drawn right now. */
export function showing(): boolean {
  return shown !== null;
}
