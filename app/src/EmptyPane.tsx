// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { FolderPlus, MessageSquarePlus } from "lucide-react";
import type { Offer } from "./actions";
import { Doer } from "./StripMarks";
import { EmptyState } from "./ui/empty-state/EmptyState";

/**
 * **A pane that shows nothing** (#4755, spec #4750 story 10; #5292, spec #5288 stories 84, 85
 * and 89): it says it holds nothing yet, offers New chat and New shell, says a chat can be dragged in
 * from the sidebar, and how many chats run in its workspace. A pane shows a chat, a view, or this,
 * so an empty window is never a dead end. Alone in its tab and as one side of a split, it says
 * the same, as the moving-panes prototype draws it (accepted 2026-11-05). Its headline is a claim,
 * as `docs/ui-copy.md` asks of every empty state, where the prototype asked "What goes here?".
 *
 * New chat and New shell are the catalogue's `chat.new` and `shell.new` rows drawn by `Doer`, as
 * the centre's own empty states draw them, so each stops existing where the catalogue stops
 * offering it. A press focuses this pane first, so the one rule fills it (`tabs.opensAt`). Their
 * words are the empty state's ("New chat"), not the strip's `+` ("New tab"): two controls on one
 * screen with one accessible name are two a screen reader cannot tell apart.
 *
 * **In a plane with no workspace yet** (#4758; spec story 11) it says so, and offers Create a
 * workspace first: one empty pane replaces the window's three empty states, and this was the
 * one with a first step of its own.
 */
export function EmptyPane({
  running,
  newChat,
  newShell,
  onPress,
  createWorkspace,
}: {
  /** How many chats run in this pane's workspace. */
  running: number;
  /** The catalogue's `chat.new` row, where it offers one. */
  newChat: Offer | undefined;
  /** The catalogue's `shell.new` row, where it offers one. */
  newShell: Offer | undefined;
  onPress: (offer: Offer) => void;
  /** The catalogue's `workspace.create` row, given only while the plane has no workspace. */
  createWorkspace?: Offer;
}) {
  if (createWorkspace !== undefined)
    return (
      <EmptyState
        mark={FolderPlus}
        headline="No workspaces yet"
        body="A workspace holds the repos you work on and the chats about them."
        action={
          <>
            <Doer offer={createWorkspace} onPress={onPress} words="Create a workspace" primary />
            <Doer offer={newChat} onPress={onPress} words="New chat" />
          </>
        }
        testid="empty-pane"
      />
    );
  return (
    <EmptyState
      mark={MessageSquarePlus}
      headline="Nothing in this pane yet"
      body={running === 0 ? runningHere(running) : `${DRAG_ONE_IN} ${runningHere(running)}`}
      action={
        <>
          <Doer offer={newChat} onPress={onPress} words="New chat" primary />
          <Doer offer={newShell} onPress={onPress} words="New shell" />
        </>
      }
      testid="empty-pane"
    />
  );
}

/** **A chat can be dragged in** (spec #5288 story 85): said only while one runs here to drag. */
const DRAG_ONE_IN = "Drag a chat here from the sidebar, or start one.";

/**
 * How many chats run here, in a sentence. **The body says a count**, because the spec asks for
 * it (#4750 story 10), after the way in that the list is ({@link DRAG_ONE_IN}); the other ways
 * out are the buttons under it.
 */
export function runningHere(running: number): string {
  if (running === 0) return "Nothing runs in this workspace yet.";
  return running === 1 ? "1 chat is running here." : `${running} chats are running here.`;
}
