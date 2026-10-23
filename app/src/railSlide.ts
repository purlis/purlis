// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The project rail sliding** (#2915, R84): while its column opens or closes, the work beside it
 * is a little wider each frame. What fits itself to its room — a chat's terminal, which tells its
 * program every size it is fitted to — waits until the slide is over and fits once, so a program
 * is told one new size per hide or show, not one a frame.
 *
 * Window-wide state, as the slide is the window's: the rail starts it as its column starts to move
 * ({@link slideStarted}) and ends it when the column's transition ends, or its length has passed
 * ({@link slideEnded}).
 */

let sliding = false;
/** What waits for the slide's end, each once however often it asked. */
const waiting = new Set<() => void>();

/** The rail's column has started to open or close. */
export function slideStarted(): void {
  sliding = true;
}

/** The rail's column is still: everything that waited runs, once each. */
export function slideEnded(): void {
  if (!sliding) return;
  sliding = false;
  const due = [...waiting];
  waiting.clear();
  for (const run of due) run();
}

/** Whether the rail's column is moving now. */
export function railSliding(): boolean {
  return sliding;
}

/**
 * Runs `refit` now, or once the slide is over when the rail is moving: the same function asked
 * for on every frame of a slide runs once at its end.
 */
export function afterTheSlide(refit: () => void): void {
  if (sliding) waiting.add(refit);
  else refit();
}

/** Forgets a slide a test left unfinished, and what waited for it. For tests. */
export function forgetTheSlide(): void {
  sliding = false;
  waiting.clear();
}
