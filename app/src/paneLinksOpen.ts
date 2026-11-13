// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where the window reaches a pane's *Links in this pane…*** (#6332). The list is the pane's
 * own (`usePaneLinks`), and the window's rows for it, the palette's and each pane's ⋯ and head
 * rows, are the catalogue's (`actions.ts`, `listLinks`). A pane that draws a chat or a shell says
 * here, while it is drawn, how to open its list; the window opens the one that draws the chat
 * the row is about. One per chat: the window draws a chat in one pane.
 */
const opens = new Map<string, () => void>();

const keyOf = (plane: string, session: number) => `${plane}\n${session}`;

/** Says how to open the list of the pane drawing `session`, until the answer is called. */
export function listsLinks(plane: string, session: number, open: () => void): () => void {
  const key = keyOf(plane, session);
  opens.set(key, open);
  return () => {
    // A pane drawn again in its place may already have said its own.
    if (opens.get(key) === open) opens.delete(key);
  };
}

/** Opens the list of the pane drawing `session`, and answers whether one did. */
export function listLinksOf(plane: string, session: number): boolean {
  const open = opens.get(keyOf(plane, session));
  open?.();
  return open !== undefined;
}
