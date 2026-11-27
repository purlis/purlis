// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where the window reaches a pane's *Links in this pane…*** (#6332). The list is the pane's
 * own (`usePaneLinks`), and the window's rows for it, the palette's and each pane's ⋯ and head
 * rows, are the catalogue's (`actions.ts`, `listLinks`). A pane that draws a chat or a shell says
 * here, while it is drawn, how to open its list; the window opens the one that draws the chat
 * the row is about.
 *
 * **One chat may be drawn in two panes for a moment** (#6488): a pane drawn again in its place,
 * or a chat moved while its old pane is still going. Each pane's word is kept, newest last, and
 * the newest pane still drawn opens: one that goes takes only its own word with it.
 */
const opens = new Map<string, (() => void)[]>();

const keyOf = (plane: string, session: number) => `${plane}\n${session}`;

/** What the window says when no pane drawing the chat has said how to open its list (#6488):
 *  none shows it yet, or the one that does has no core to list links with. */
export const LINKS_UNLISTED =
  "purlis cannot list this chat's links: none of its panes can list them yet. Show the chat in a pane, then try again.";

/** Says how to open the list of the pane drawing `session`, until the answer is called. */
export function listsLinks(plane: string, session: number, open: () => void): () => void {
  const key = keyOf(plane, session);
  // Its own entry, so a pane that says the same function twice takes back only one of them.
  const said = () => open();
  opens.set(key, [...(opens.get(key) ?? []), said]);
  return () => {
    const left = (opens.get(key) ?? []).filter((one) => one !== said);
    if (left.length === 0) opens.delete(key);
    else opens.set(key, left);
  };
}

/** Opens the list of the newest pane drawing `session`, and answers whether one did. */
export function listLinksOf(plane: string, session: number): boolean {
  const open = opens.get(keyOf(plane, session))?.at(-1);
  open?.();
  return open !== undefined;
}
