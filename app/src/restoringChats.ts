// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { OpenChat } from "./bindings";

/**
 * **The chats a launch has still to start** (`restoring.rs`), as the window keeps them: by the
 * number each one's tab is drawn under, which is the number it starts as.
 *
 * A launch registers every chat it puts back and starts them behind the window, a few at a
 * time, so the window can be used at once however many there are. Each has its tab in its
 * place from the first frame; until its program has started there is no terminal to draw, and
 * its pane says so (`RestoringPane`).
 */
export const NONE_RESTORING: ReadonlySet<number> = new Set();

/** What a restoring chat's tab says of it: its tooltip. */
export const RESTORING = "Restoring…";

/** Why a restoring chat has no close yet: its row's reason in the catalogue. */
export const RESTORING_HAS_NO_CLOSE =
  "This chat is still restoring. It can be ended once it has started.";

/** The event the core sends as each chat of a launch starts, or is found not to. */
export const CHAT_RESTORED = "chat-restored";

/** What {@link CHAT_RESTORED} carries (`restoring::ChatRestored`). */
export type ChatRestored = {
  plane: string;
  /** The number its tab was drawn under while it was restoring. */
  number: number;
  /** The session it started as, or `null` where it did not start. */
  session: number | null;
  /** How many of the project's chats are still to start. */
  left: number;
};

/** Whether the core says `chat` is one a launch has still to start. */
export function isRestoring(chat: Pick<OpenChat, "restoring">): boolean {
  return chat.restoring !== undefined && chat.restoring !== null;
}

/**
 * What a read of the project's chats says has become of the chats the window drew as
 * restoring: which have started, which are gone from the list (they did not start, and are
 * among the chats that would not), and which are restoring still.
 *
 * `said` is what an event said of one chat: the number its tab was drawn under and the session
 * it started as. Where those differ and the tab is still drawn under the old number, the chat is
 * `renumbered`: the same chat under a new number, put into the tab it was drawn in (#4962), so
 * the tab keeps its pin, its place and its panes. Where the old number's tab is gone already
 * (an earlier read found it gone), the chat is `arrived`: it is drawn as a chat that has just
 * been started.
 */
export function restoredSince(
  was: ReadonlySet<number>,
  open: readonly OpenChat[],
  said: Pick<ChatRestored, "number" | "session"> | null,
  shown: (session: number) => boolean,
): {
  still: Set<number>;
  started: OpenChat[];
  gone: number[];
  arrived: OpenChat[];
  renumbered: { from: number; chat: OpenChat }[];
} {
  // Only ever fewer: the launch draws every one at once, and a chat that has started never
  // restores again. So an answer older than one already read, which `opened_chats` answering
  // off the window's thread can land late (#4739), draws no started chat as restoring again.
  const still = new Set(
    open.filter((chat) => isRestoring(chat) && was.has(chat.session)).map((chat) => chat.session),
  );
  const started = open.filter((chat) => !isRestoring(chat) && was.has(chat.session));
  const listed = (number: number) => open.some((chat) => chat.session === number);
  const named = said?.session ?? null;
  const now =
    named === null || was.has(named)
      ? []
      : open.filter((chat) => chat.session === named && !isRestoring(chat) && !shown(chat.session));
  const from = said !== null && was.has(said.number) && !listed(said.number) ? said.number : null;
  const renumbered = from === null ? [] : now.map((chat) => ({ from, chat }));
  const gone = [...was].filter(
    (number) => !listed(number) && !renumbered.some((one) => one.from === number),
  );
  const arrived = renumbered.length > 0 ? [] : now;
  return { still, started, gone, arrived, renumbered };
}
