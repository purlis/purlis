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

/** How often a window with chats still restoring reads them again (`readRestored`). */
export const RESTORE_LOOKS_EVERY = 3000;

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

/** What an event said of one chat: the number its tab was drawn under, and the session it
 *  started as. */
export type RestoreSaid = Pick<ChatRestored, "number" | "session">;

/** A chat that started under a new number, and the number its tab was drawn under. */
export type Renumbered = { from: number; chat: OpenChat };

/**
 * What the reads of the project's chats have seen so far, for the next to go on from: the order
 * the last one listed them in, and each number drawn as restoring that a read found missing
 * without an event naming it, with how many of the window's own looks have found it so.
 */
export type RestoreSeen = {
  order: readonly number[];
  missedLooks: ReadonlyMap<number, number>;
};

/** What a launch's first read has seen: the chats in the order it listed them. */
export function seenFirst(open: readonly OpenChat[]): RestoreSeen {
  return { order: open.map((chat) => chat.session), missedLooks: new Map() };
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
 * - **A read finds it before the event does**, in its place on the list ({@link inItsPlace}).
 *
 * A number a read finds missing that neither says is **held**, still drawn as restoring, for the
 * event to say whether it started under a new number or did not start. Only an event that says
 * it did not start makes it `gone`, or, where that event never comes (it went to another
 * window), a second look of the window's own (`said` is `null`) that finds it missing with
 * nothing else restoring. A read an event of another chat asks for never closes it: the events
 * of several chats can be heard in any order.
 *
 * Where an event names a session whose old tab is gone already, the chat is `arrived`: it is
 * drawn as a chat that has just been started.
 */
export function restoredSince(
  was: ReadonlySet<number>,
  open: readonly OpenChat[],
  said: RestoreSaid | null,
  shown: (session: number) => boolean,
  seen: RestoreSeen = seenFirst([]),
): {
  still: Set<number>;
  started: OpenChat[];
  gone: number[];
  arrived: OpenChat[];
  renumbered: Renumbered[];
  seen: RestoreSeen;
} {
  // Only ever fewer: the launch draws every one at once, and a chat that has started never
  // restores again. So an answer older than one already read, which `opened_chats` answering
  // off the window's thread can land late (#4739), draws no started chat as restoring again.
  const still = new Set(
    open.filter((chat) => isRestoring(chat) && was.has(chat.session)).map((chat) => chat.session),
  );
  const othersRestoring = still.size > 0;
  const started = open.filter((chat) => !isRestoring(chat) && was.has(chat.session));
  const listed = (number: number) => open.some((chat) => chat.session === number);
  /** A chat that has started and that no tab shows: one a renumbered tab can take. */
  const unshown = (chat: OpenChat | undefined): chat is OpenChat =>
    chat !== undefined && !isRestoring(chat) && !was.has(chat.session) && !shown(chat.session);
  const gone: number[] = [];
  const renumbered: Renumbered[] = [];
  const missedLooks = new Map<number, number>();
  for (const number of was) {
    if (listed(number)) continue;
    if (said !== null && said.number === number) {
      const chat = open.find((one) => one.session === said.session);
      if (said.session !== number && unshown(chat)) renumbered.push({ from: number, chat });
      else gone.push(number);
      continue;
    }
    const before = seen.missedLooks.get(number);
    if (before === undefined) {
      const chat = inItsPlace(number, open, seen.order);
      if (unshown(chat) && !renumbered.some((one) => one.chat === chat)) {
        renumbered.push({ from: number, chat });
        continue;
      }
    }
    const looks = (before ?? 0) + (said === null ? 1 : 0);
    if (said === null && looks >= 2 && !othersRestoring) gone.push(number);
    else {
      missedLooks.set(number, looks);
      still.add(number);
    }
  }
  const arrived =
    said === null || said.session === null || was.has(said.number) || was.has(said.session)
      ? []
      : open.filter(
          (chat) => chat.session === said.session && !isRestoring(chat) && !shown(chat.session),
        );
  return {
    still,
    started,
    gone,
    arrived,
    renumbered,
    seen: { order: open.map((chat) => chat.session), missedLooks },
  };
}

/**
 * The chat in `number`'s place on the list, where only one can be: the core puts a chat that
 * started under a new number where the old number was (`Chats::finish_put_back`). Between the
 * nearest chats listed both times on either side of it, the read before listed `number` alone
 * and this one lists one chat it did not list before. Where two neighbours went together (one
 * that did not start beside one that started anew) no place is sure, and none is answered.
 */
export function inItsPlace(
  number: number,
  open: readonly OpenChat[],
  order: readonly number[],
): OpenChat | undefined {
  const at = order.indexOf(number);
  if (at < 0) return undefined;
  const kept = (session: number) => open.some((chat) => chat.session === session);
  let ahead = at - 1;
  while (ahead >= 0 && !kept(order[ahead])) ahead -= 1;
  const behind = order.slice(at + 1).findIndex(kept);
  const gapBefore = order.slice(ahead + 1, behind < 0 ? order.length : at + 1 + behind);
  const from = ahead < 0 ? 0 : open.findIndex((chat) => chat.session === order[ahead]) + 1;
  const to =
    behind < 0 ? open.length : open.findIndex((chat) => chat.session === order[at + 1 + behind]);
  const gapNow = open.slice(from, to);
  if (gapBefore.length !== 1 || gapNow.length !== 1) return undefined;
  return order.includes(gapNow[0].session) ? undefined : gapNow[0];
}
