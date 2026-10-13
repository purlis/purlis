// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext } from "react";

import type { OpenChat } from "./bindings";

/**
 * **The chats a launch has still to start** (`restoring.rs`), by the number each one's tab is
 * drawn under, which is the number it starts as.
 *
 * A launch registers every chat it puts back and starts them behind the window, a few at a
 * time, so the window can be used at once however many there are. Each has its tab in its
 * place from the first frame; until its program has started there is no terminal to draw, and
 * its pane says so (`RestoringPane`). A context, as `TaskEndContext` is: the strip and the
 * panes are drawn several components below the view that hears each chat start.
 */
export const RestoringContext = createContext<ReadonlySet<number>>(new Set());

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
 * `named` is the session an event said a chat started as. Where that is not the number its tab
 * was drawn under, the chat is `arrived`: it is drawn as a chat that has just been started,
 * and the tab under the old number goes with the ones that are gone.
 */
export function restoredSince(
  was: ReadonlySet<number>,
  open: readonly OpenChat[],
  named: number | null,
  shown: (session: number) => boolean,
): { still: Set<number>; started: OpenChat[]; gone: number[]; arrived: OpenChat[] } {
  const still = new Set(open.filter(isRestoring).map((chat) => chat.session));
  const started = open.filter((chat) => !isRestoring(chat) && was.has(chat.session));
  const gone = [...was].filter((number) => !open.some((chat) => chat.session === number));
  const arrived =
    named === null || was.has(named)
      ? []
      : open.filter((chat) => chat.session === named && !isRestoring(chat) && !shown(chat.session));
  return { still, started, gone, arrived };
}
