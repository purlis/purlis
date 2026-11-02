// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { MessageSquarePlus } from "lucide-react";
import type { Offer } from "./actions";
import { Doer } from "./StripMarks";
import { EmptyState } from "./ui/empty-state/EmptyState";

/**
 * **A pane that shows nothing** (#4755, spec #4750 story 10): what an empty pane says, how many
 * chats run in its workspace, and New chat. A pane shows a chat, a view, or this, so an empty
 * window is never a dead end. As the one-tab-strip prototype draws it (accepted on #4582).
 *
 * New chat is the catalogue's `chat.new` row drawn by `Doer`, as the centre's own empty states
 * draw it, so it stops existing where the catalogue stops offering it. Its words are the empty
 * state's ("New chat"), not the strip's `+` ("New tab"): two controls on one screen with one
 * accessible name are two a screen reader cannot tell apart.
 */
export function EmptyPane({
  running,
  newChat,
  onPress,
  inSplit,
}: {
  /** How many chats run in this pane's workspace. */
  running: number;
  /** The catalogue's `chat.new` row, where it offers one. */
  newChat: Offer | undefined;
  onPress: (offer: Offer) => void;
  /** The pane is one side of a split, and the prototype names it apart from a tab's only pane. */
  inSplit: boolean;
}) {
  return (
    <EmptyState
      mark={MessageSquarePlus}
      headline={inSplit ? "An empty pane" : "Nothing in this pane"}
      body={runningHere(running)}
      action={<Doer offer={newChat} onPress={onPress} words="New chat" primary />}
      testid="empty-pane"
    />
  );
}

/** How many chats run here, in a sentence. */
export function runningHere(running: number): string {
  if (running === 0) return "Nothing runs in this workspace yet.";
  return running === 1 ? "1 chat is running here." : `${running} chats are running here.`;
}
