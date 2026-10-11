// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A keyboard move a palette row asks for is made once the palette has handed the keyboard
 * back** (#1769). The palette's closing move puts the keyboard back where the person opened it
 * from (`Palette.giveTheKeyboardBack`), and Radix makes that move only once the palette has
 * gone, which on a loaded machine is later than the next frame. A row that moved the keyboard
 * a frame after it ran — Focus the next region — was undone by it now and then, and the
 * keyboard was left where the palette had been opened.
 *
 * So a move is asked for here instead: made at once with no palette up, and otherwise by the
 * palette's closing move, right after it has handed the keyboard back. One that is never
 * handed on (a palette that went without its closing move) is made after {@link PATIENCE}.
 */

/** How long a move waits on a palette that never says it handed the keyboard back. */
export const PATIENCE = 1000;

let up = false;
let waiting: { move: () => void; timer: ReturnType<typeof setTimeout> } | undefined;

/** The palette is up: a move asked for now waits for its closing move. */
export function thePaletteIsUp(): void {
  up = true;
}

/** Makes `move` now, or, while the palette is up, once it has handed the keyboard back. */
export function onceTheKeyboardIsBack(move: () => void): void {
  if (!up) {
    move();
    return;
  }
  if (waiting) clearTimeout(waiting.timer);
  waiting = { move, timer: setTimeout(theKeyboardIsBack, PATIENCE) };
}

/** The palette's closing move has handed the keyboard back: the move waiting is made. */
export function theKeyboardIsBack(): void {
  up = false;
  const was = waiting;
  waiting = undefined;
  if (was) {
    clearTimeout(was.timer);
    was.move();
  }
}
