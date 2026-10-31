// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { drawIn, inForce, onDrawn, sameTheme, type Theme } from "./theme";
import { watchWindowLeft } from "../ui/windowLeft";

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
 * **A preview never outlives its card.** Every ask for a card's theme takes a ticket
 * ({@link ask}), and only the newest ticket's answer is drawn: leaving, putting back, a press
 * and the tab going each make the tickets out stale, so an answer that arrives after its card
 * is gone draws nothing. While a card is shown, the person's theme is put back when the
 * window is hidden or loses the focus, on Escape, and as soon as the cards are no longer in
 * the document (a project switch takes its view out without unmounting it).
 *
 * **Moving from one card to the next is one change, not two**: leaving puts the person's theme
 * back only after the events of the move are over, and the next card's theme replaces that
 * wait, so the window never flashes its own theme between two cards. A card whose theme is
 * the one drawn draws nothing.
 */

/** The card's theme being drawn, while one is. */
let shown: Theme | null = null;
/** What is in force underneath it. */
let back: Theme | null = null;
/** The put-back waiting for the move's events to end. */
let leaving: ReturnType<typeof setTimeout> | undefined;
/** The newest ticket: only an answer asked with it is drawn. */
let generation = 0;
/** The cards the shown theme belongs to, while they are in the document. */
let within: Element | null = null;
/** How to stop everything that watches a shown card. */
let unwatch: (() => void) | undefined;

/** How often a shown card's cards are looked for in the document. */
const LOOK_MS = 250;

/** **A ticket for one ask of a card's theme**: the answer is drawn only if no newer ask, leave
 *  or put-back came first. */
export function ask(): number {
  return ++generation;
}

/** Draws `theme`, asked with `ticket`, while its card (inside `cards`) is pointed at or
 *  focused. Answers whether it was drawn. */
export function showWhileOn(theme: Theme, ticket: number, cards?: Element | null): boolean {
  if (ticket !== generation) return false;
  if (cards != null && !cards.isConnected) return false;
  if (leaving !== undefined) {
    clearTimeout(leaving);
    leaving = undefined;
  }
  // The theme drawn already: nothing to draw, and nothing to put back.
  const under = shown === null ? inForce() : back;
  if (under !== null && sameTheme(theme, under)) {
    restore();
    return false;
  }
  if (shown === null) {
    back = inForce();
    watch();
  }
  within = cards ?? within;
  shown = theme;
  if (inForce() !== theme) drawIn(theme);
  return true;
}

/** Puts the person's theme back once the events of the move are over: unless another card is
 *  shown first. An answer for the card left is drawn no more. */
export function putBackSoon(): void {
  generation++;
  if (shown === null || leaving !== undefined) return;
  leaving = setTimeout(restore, 0);
}

/** Puts the person's theme back at once, and draws no answer still out: the card was left for
 *  good (the tab went, a choice was made, Escape). */
export function putBack(): void {
  generation++;
  restore();
}

/** Whether a card's theme is drawn right now. */
export function showing(): boolean {
  return shown !== null;
}

/** Draws what is in force underneath again, and stops watching. */
function restore(): void {
  if (leaving !== undefined) {
    clearTimeout(leaving);
    leaving = undefined;
  }
  if (shown === null) return;
  const to = back;
  shown = null;
  back = null;
  within = null;
  unwatch?.();
  unwatch = undefined;
  if (to !== null && to !== inForce()) drawIn(to);
}

/** While a card is shown: the window's own draws become what goes back, and the window
 *  hidden, unfocused, Escape or the cards gone put the person's theme back. */
function watch(): void {
  const stopDrawn = onDrawn((drawn) => {
    if (shown === null || drawn === shown) return;
    back = drawn;
    if (within !== null && !within.isConnected) {
      putBack();
      return;
    }
    // After that draw has reached every follower, so none is left on the window's own.
    queueMicrotask(() => {
      if (shown !== null && inForce() !== shown) drawIn(shown);
    });
  });
  const hidden = () => {
    if (document.visibilityState === "hidden") putBack();
  };
  const escape = (event: KeyboardEvent) => {
    if (event.key === "Escape") putBack();
  };
  const gone = setInterval(() => {
    if (within !== null && !within.isConnected) putBack();
  }, LOOK_MS);
  document.addEventListener("visibilitychange", hidden);
  // The window from the front: the pointer gone from it is the gallery's to weigh, as the keyboard
  // may still be on the card (#3095).
  const unwatchLeft = watchWindowLeft((why) => {
    if (why === "focus") putBack();
  });
  document.addEventListener("keydown", escape, true);
  unwatch = () => {
    stopDrawn();
    clearInterval(gone);
    document.removeEventListener("visibilitychange", hidden);
    unwatchLeft();
    document.removeEventListener("keydown", escape, true);
  };
}
