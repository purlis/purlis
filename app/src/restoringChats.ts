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
 * What the reads of the project's chats have seen so far, for the next to go on from: the order
 * the last one listed them in, and each number drawn as restoring that a read found missing
 * without an event naming it, with how many reads have found it so.
 */
export type RestoreSeen = {
  order: readonly number[];
  missing: ReadonlyMap<number, number>;
};

/** What a launch's first read has seen: the chats in the order it listed them. */
export function seenFirst(open: readonly OpenChat[]): RestoreSeen {
  return { order: open.map((chat) => chat.session), missing: new Map() };
}

/**
 * What a read of the project's chats says has become of the chats the window drew as
 * restoring: which have started, which are gone from the list (they did not start, and are
 * among the chats that would not), and which are restoring still.
 *
 * **A chat that started under a new number keeps its tab** (#4962): it is `renumbered`, and the
 * window puts the new number into the tab it was drawn in, so the tab keeps its pin, its place
 * and its panes. It is known so two ways:
 *
 * - **An event names it** (`said`): the number its tab was drawn under, and the session it
 *   started as.
 * - **A read finds it before the event does**: its number is no longer listed, and in its
 *   place on the list (the core puts the new session where the old number was) is a session the
 *   read before did not list, started, and that no tab shows.
 *
 * A number a read finds missing that neither says is **held**, still drawn as restoring, for the
 * event to say whether it started under a new number or did not start. Only an event that says
 * it did not start makes it `gone`, or, where that event never comes, a look of the window's
 * own (`said` is `null`) that finds it missing again with nothing else restoring.
 *
 * Where an event names a session whose old tab is gone already, the chat is `arrived`: it is
 * drawn as a chat that has just been started.
 */
export function restoredSince(
  was: ReadonlySet<number>,
  open: readonly OpenChat[],
  said: Pick<ChatRestored, "number" | "session"> | null,
  shown: (session: number) => boolean,
  seen: RestoreSeen = { order: [], missing: new Map() },
): {
  still: Set<number>;
  started: OpenChat[];
  gone: number[];
  arrived: OpenChat[];
  renumbered: { from: number; chat: OpenChat }[];
  seen: RestoreSeen;
} {
  // Only ever fewer: the launch draws every one at once, and a chat that has started never
  // restores again. So an answer older than one already read, which `opened_chats` answering
  // off the window's thread can land late (#4739), draws no started chat as restoring again.
  const still = new Set(
    open.filter((chat) => isRestoring(chat) && was.has(chat.session)).map((chat) => chat.session),
  );
  const restoringElse = still.size > 0;
  const started = open.filter((chat) => !isRestoring(chat) && was.has(chat.session));
  const listed = (number: number) => open.some((chat) => chat.session === number);
  /** A chat that has started and that no tab shows: one a renumbered tab can take. */
  const unshown = (chat: OpenChat | undefined): chat is OpenChat =>
    chat !== undefined && !isRestoring(chat) && !was.has(chat.session) && !shown(chat.session);
  /** And, found by its place alone, one the read before did not list and no other has taken. */
  const fresh = (chat: OpenChat | undefined, claimed: readonly OpenChat[]): chat is OpenChat =>
    unshown(chat) && !seen.order.includes(chat.session) && !claimed.includes(chat);
  const named = said?.session ?? null;
  const gone: number[] = [];
  const renumbered: { from: number; chat: OpenChat }[] = [];
  const missing = new Map<number, number>();
  for (const number of was) {
    if (listed(number)) continue;
    if (said?.number === number) {
      const chat = open.find((one) => one.session === named);
      if (named !== null && named !== number && unshown(chat)) {
        renumbered.push({ from: number, chat });
      } else gone.push(number);
      continue;
    }
    const times = seen.missing.get(number);
    if (times === undefined) {
      // In its place on the list: after the nearest chat the read before listed ahead of it
      // that this one lists still.
      const ahead = seen.order.slice(0, Math.max(seen.order.indexOf(number), 0));
      const after = [...ahead].reverse().find(listed);
      const at = after === undefined ? 0 : open.findIndex((one) => one.session === after) + 1;
      const chat = open[at];
      const claimed = renumbered.map((one) => one.chat);
      if (seen.order.includes(number) && fresh(chat, claimed)) {
        renumbered.push({ from: number, chat });
        continue;
      }
    }
    const now = (times ?? 0) + 1;
    // Its event lost (it went to another window): a look of the window's own, after one that
    // found it missing already, with nothing else restoring, takes it as one that did not start.
    if (said === null && now >= 2 && !restoringElse) gone.push(number);
    else {
      missing.set(number, now);
      still.add(number);
    }
  }
  const arrived =
    named === null || was.has(said?.number ?? NaN) || was.has(named)
      ? []
      : open.filter((chat) => chat.session === named && !isRestoring(chat) && !shown(chat.session));
  return {
    still,
    started,
    gone,
    arrived,
    renumbered,
    seen: { order: open.map((chat) => chat.session), missing },
  };
}
