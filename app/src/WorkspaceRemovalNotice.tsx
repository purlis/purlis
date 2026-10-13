// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useState } from "react";
import type { PlaneId, Shown } from "./bindings";
import { OPENS_DELETE_WORKSPACE, answerTaken, asksMoved } from "./asks";
import { Notice, NoticeOf, type NoticeAction } from "./Notice";

/** How many of a thing, in words: `no memories`, `1 memory`, `12 memories`. */
const counted = (count: number, one: string, many: string) =>
  count === 0 ? `no ${many}` : count === 1 ? `1 ${one}` : `${count} ${many}`;

/**
 * **What a workspace holds that a removal deletes**, as far as the core counted it: its
 * memories and its open todos. Its clones are named and never counted, since what matters of a
 * clone is work nothing else has, and that is Delete workspace's to list.
 */
export function holdsSaid(memories: number, todos: number): string {
  return `It holds ${counted(memories, "memory", "memories")} and ${counted(todos, "open todo", "open todos")}.`;
}

/**
 * **A workspace this chat asked you to remove, on its tab** (qw356). A chat whose sandbox
 * holds a removal is refused `purlis workspace remove`, and that was a dead end even in the
 * chat Safe remove opens. Now the refusal asks here: which workspace, what removing it deletes,
 * and
 *
 * - **Remove…**: opens the window's own Delete workspace on it and removes nothing itself.
 *   That dialog says what goes and that there is no undo, lists work nothing else has, and
 *   its Delete is the press that removes, with the core's guard. The question waits until the
 *   workspace is removed there, or kept here.
 * - **Keep**: puts the question away and removes nothing.
 *
 * A removal is for good, and it is a chat that raised this, so no single press on a Notice or
 * an Inbox row removes anything: the chat handed over a name, the core looked it up in this
 * project's own list, and the person decides in the dialog they would have opened from the
 * sidebar. Every word is purlis's; the name is drawn as data. The chat is told what you
 * decided on its next turn.
 *
 * Drawn from the asks registry (`asks`), so an answer in the Inbox clears it here, and its
 * answers carry the registry's labels.
 */
export function WorkspaceRemovalNotice({
  plane,
  session,
  asks,
  onWay,
}: {
  plane: PlaneId;
  session: number;
  /** This chat's asks its pane draws, of which the removals are taken. */
  asks: readonly Shown[];
  /** An answer was pressed: its words, and the ask it answered. */
  onWay?: (words: string, ask: Shown) => void;
}) {
  /** "this chat" on its own pane; "it" where its path is said first (#1538). */
  const offScreen = useContext(NoticeOf) !== null;
  /** What the last press was refused with, by the ask it was pressed on. */
  const [refused, setRefused] = useState<Readonly<Record<string, string>>>({});
  const [busy, setBusy] = useState<string>();
  return (
    <>
      {asks.map((ask) => {
        const path = ask.answer;
        if (ask.session !== session || path.via !== "workspace-removal") return null;
        const [first, ...rest] = ask.options.map((option): NoticeAction => ({
          label: option.label,
          busy: busy === ask.ask,
          onPress: () => {
            if (busy !== undefined) return;
            setBusy(ask.ask);
            setRefused((was) =>
              Object.fromEntries(Object.entries(was).filter(([key]) => key !== ask.ask)),
            );
            // Opening Delete workspace answers nothing, so nothing is on its way to the chat.
            if (option.id !== OPENS_DELETE_WORKSPACE) onWay?.(option.label, ask);
            void answerTaken(plane, ask, option.id)
              .then((answer) => {
                const why = answer.refused;
                if (why !== undefined) setRefused((was) => ({ ...was, [ask.ask]: why }));
              })
              .finally(() => {
                setBusy(undefined);
                asksMoved(plane);
              });
          },
        }));
        if (first === undefined) return null;
        const said = refused[ask.ask];
        return (
          <Notice
            key={ask.ask}
            cause={`workspace-removal:${session}:${path.workspace}`}
            at="pane"
            tone={said === undefined ? "news" : "trouble"}
            label="Remove workspace"
            fixes={[first, ...rest]}
          >
            {offScreen ? "It asks" : "This chat asks"} you to remove workspace{" "}
            <code className="block-allow-target">{path.workspace}</code>. Removing it deletes its
            memory, its todos and its clones. {holdsSaid(path.memories, path.todos)} {first.label}{" "}
            opens Delete workspace, where you decide: nothing is removed here, and{" "}
            {offScreen ? "it" : "this chat"} is told what you decided on its next turn.
            {said !== undefined && ` ${said}`}
          </Notice>
        );
      })}
    </>
  );
}
