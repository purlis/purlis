// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where phase 1a's model key Asks mark a tab** (T1a-61, R-592).
 *
 * - **A no-chat Ask names its chats.** A key missing or unmarked, and an old profile's key to
 *   copy, are keyed by what is missing, not by a chat, so Asks alone draws them; each carries the
 *   chats it names (`Shown.names`: the chats it stopped, or the profile's chats), and the window
 *   marks each named chat's tab while the Ask stands. A chat closed meanwhile is dropped.
 * - **Your keys locked at a start** stands in place in New chat while the person is there, and
 *   marks no tab; only an unattended start's goes to Needs you, marking its tab.
 */
import type { Shown } from "./bindings";

/** The no-chat Ask kinds that mark the tabs of the chats they name. */
const NAMES_ITS_CHATS: ReadonlySet<Shown["source"]> = new Set<Shown["source"]>([
  "model-key-missing",
  "model-key-unmarked",
  "model-key-copy",
]);

/** **The tabs `asks` mark**, of the chats still `open`: each chat a no-chat model key Ask names. */
export function tabsMarked(asks: readonly Shown[], open: readonly number[]): ReadonlySet<number> {
  const opened = new Set(open);
  const marked = new Set<number>();
  for (const ask of asks) {
    if (!NAMES_ITS_CHATS.has(ask.source)) continue;
    for (const chat of ask.names ?? []) if (opened.has(chat)) marked.add(chat);
  }
  return marked;
}

/** **Whether an Ask in `asks` names chat `session`**, so its tab is marked. */
export function namedByAnAsk(asks: readonly Shown[] | undefined, session: number): boolean {
  return tabsMarked(asks ?? [], [session]).has(session);
}

/** **Where your keys locked at a start stands**, and whether it marks its chat's tab. */
export function unlockAskAt(attended: boolean): { at: "new-chat" | "needs-you"; marksTab: boolean } {
  return attended ? { at: "new-chat", marksTab: false } : { at: "needs-you", marksTab: true };
}
