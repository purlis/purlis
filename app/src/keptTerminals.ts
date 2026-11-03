// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The last chats shown keep their terminals** (#4735, part of #4724).
 *
 * A tab switch draws the pane again for another chat (`SessionPane`). It used to dispose the
 * terminal of the chat it left and close that chat's view, so switching back started a new
 * terminal, opened a new view and was sent the screen again: a person felt that as a pane that
 * took its time to come back, and as keys that lagged until it had.
 *
 * So the last {@link KEPT_CHATS} chats shown across the window, in every project, keep their
 * terminal and their view when the pane leaves them. A kept terminal is held here, its element
 * in no document, the way a kept project's view is (`projectShown.tsx`): nothing lays it out or
 * paints it, and it goes on taking what its program writes. Switching back puts that element
 * in the pane again, with its screen as it is now and nothing to start or replay.
 *
 * **The one in front counts among them.** Showing a fourth chat lets go of the least recently
 * shown one: its terminal is disposed and its view closed, and showing it again starts a new one
 * as before. What the window keeps is bounded by this and not by how many chats it holds.
 *
 * A chat whose pane is still drawn, in a project kept off the window, is not held here: its
 * pane holds it. Only one whose pane let go of it is.
 */
export const KEPT_CHATS = 3;

/** A terminal held here, and how to let go of it: dispose it and close its view. */
export interface KeptTerminal {
  release(): void;
}

/** The chats shown, the most recent first, at most {@link KEPT_CHATS} of them. */
const recent: string[] = [];
/** The kept terminals no pane is drawing, by chat. */
const held = new Map<string, KeptTerminal>();

/** The name of chat `session` of `plane` across the window: a session number is only half. */
export const keptKey = (plane: string, session: number) => `${plane}\n${session}`;

/**
 * Chat `key` is on screen. It becomes the most recently shown, and one that falls out of the
 * last {@link KEPT_CHATS} is let go of, should it be held here.
 */
export function chatShown(key: string): void {
  const at = recent.indexOf(key);
  if (at >= 0) recent.splice(at, 1);
  recent.unshift(key);
  for (const out of recent.splice(KEPT_CHATS)) {
    held.get(out)?.release();
    held.delete(out);
  }
}

/**
 * Offers chat `key`'s terminal as its pane leaves it. Answers whether it is kept; one that is
 * not, the pane lets go of itself: a chat not among the last shown, or one already held.
 */
export function keepTerminal(key: string, terminal: KeptTerminal): boolean {
  if (!recent.includes(key) || held.has(key)) return false;
  held.set(key, terminal);
  return true;
}

/** Takes chat `key`'s kept terminal back for a pane to draw, or none if it is not held. */
export function takeKeptTerminal<T extends KeptTerminal>(key: string): T | undefined {
  const terminal = held.get(key);
  held.delete(key);
  return terminal as T | undefined;
}

/** Lets go of every kept terminal and forgets what was shown: between tests. */
export function letGoOfKeptTerminals(): void {
  for (const terminal of held.values()) terminal.release();
  held.clear();
  recent.length = 0;
}
