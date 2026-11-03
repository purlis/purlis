// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { answerId, briefId, smartCloseCancelId, stopId } from "./actions";
import type { ListedChat } from "./chatsTree";
import type { SandboxSaid } from "./SandboxMark";

/**
 * **The pane head's model** (#4760, spec #4750 stories 54, 55 and 61; `command-table.md` of
 * #4585, rule 4): which one of a chat's rows the head shows as its state button, and which of
 * its parts a narrow pane drops. Pure, so the rules are read and tested here, and the head
 * (`LayoutPanes.PaneFrame`) only draws what these answer.
 */

/** **What a chat's pane head says of it** (#4760): the chat as the Chats list lists it (its
 *  name, harness, workspace and kind), its sandbox, and whether it is wrapping up. */
export type ChatHead = {
  chat: ListedChat;
  sandbox: SandboxSaid | null;
  /** Being smart-closed (ADR 0064): the head breathes, and its state button is Cancel. */
  wrapping: boolean;
};

/** The one state button: a row of the chat's own list, and the short word the head says. */
export type StateButton = { id: string; word: string };

/**
 * **The one state button a pane head shows** (variant A of #4585, accepted 2026-11-02), chosen
 * by the chat's state, first that holds:
 *
 * 1. **Cancel smart close** while it wraps up (ADR 0064): its chip on the strip is gone, and
 *    this is where it is stopped from.
 * 2. **Answer** while a task asks.
 * 3. **Brief** on any other task. A task's own stop is Stop and get its report, which is in its
 *    list (the head's Stop and Close now buttons became ⋯ rows), so a working task shows Brief
 *    as the prototype does.
 * 4. **Stop** while a chat or a shell works.
 * 5. **Nothing** on a quiet chat.
 *
 * `has` is whether the chat's list has the row now: the button is that row, so it is never one
 * the list does not have (a chat that is restoring, say), and it reads the catalogue's offer.
 */
export function stateButtonOf(chat: {
  session: number;
  task: boolean;
  working: boolean;
  has: (id: string) => boolean;
}): StateButton | undefined {
  const { session, task, working, has } = chat;
  const picks: StateButton[] = [
    { id: smartCloseCancelId(session), word: "Cancel smart close" },
    { id: answerId(session), word: "Answer" },
    ...(task ? [{ id: briefId(session), word: "Brief" }] : []),
    ...(!task && working ? [{ id: stopId(session), word: "Stop" }] : []),
  ];
  return picks.find((pick) => has(pick.id));
}

/**
 * The parts of a pane head a narrow pane can drop. The bell, ⋯, Split right and Close are not
 * among them: they never go.
 *
 * - `where`: "harness · workspace", with the line's chips (its sandbox, where a handed-off
 *   chat came from, its work item, its gauge), which are the same kind of detail.
 * - `markWord`: the state mark's word, its dot staying.
 * - `buttonWord`: the state button's word, its icon staying with the word as its name.
 * - `mark`: the state mark, dot and all.
 * - `splitDown`: the Split down button.
 */
export type HeadPart = "where" | "markWord" | "buttonWord" | "mark" | "splitDown";

/** **The ruled order**, and the pane width in CSS pixels under which each part goes (#4585). */
export const HEAD_DROPS: readonly { part: HeadPart; below: number }[] = [
  { part: "where", below: 520 },
  { part: "markWord", below: 470 },
  { part: "buttonWord", below: 420 },
  { part: "mark", below: 380 },
  { part: "splitDown", below: 300 },
];

/** The parts a pane `width` pixels wide drops, in the ruled order: none where it is not
 *  measured, as in a test with no layout. */
export function droppedAt(width: number | undefined): ReadonlySet<HeadPart> {
  if (width === undefined) return new Set();
  return new Set(HEAD_DROPS.filter((drop) => width < drop.below).map((drop) => drop.part));
}
