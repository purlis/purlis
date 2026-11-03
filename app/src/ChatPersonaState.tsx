// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo } from "react";
import { SquareTerminal } from "lucide-react";
import { markOf, ownWorkOf, useChatsHere, useChatsSelect } from "./chatState";
import { PersonaMark } from "./PersonaMark";
import { shownState, type Shown, type TaskFacts } from "./shownState";
import { StateShown } from "./StateShown";
import { useQuestionStanding } from "./QuestionsHere";
import { tasksAtWorkOf } from "./sessionTasks";
import { useTasksBelow } from "./TasksBelow";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** Whether two shown states say the same thing. */
export function sameShown(one: Shown | undefined, other: Shown | undefined): boolean {
  return one === other || (one?.kind === other?.kind && one?.word === other?.word);
}

/** What a chat's shown state is read from beside the board: its row's facts (`RowFacts`). */
export type ShownOf = {
  session: number;
  /** A shell tab, which shows no state until something reports one. */
  shell: boolean;
  /** As a task, the report it owes (`TaskFacts`); nothing for a chat that is not one. */
  report?: TaskFacts["report"] | null;
  /** How it reported, where it has and the app knows. */
  outcome?: string | null;
  /** Its harness as the person calls it: named in what is not known of it. */
  harness: string | null;
};

/**
 * **A chat's state, as every place that says it reads it** (#1484, #4935): its row in the
 * Chats list, its persona's badge and its tab, through the one function (`shownState`), so they
 * say one word for it. Subscribed to its own chat, the queue's answer about it, its question
 * and its tasks below, and changed only when what it shows changes (`sameShown`).
 *
 * **A chat waiting on its tasks says so, and how many** (#1491): it reads its own tasks
 * (`useTasksBelow`) and counts the ones that have not finished, by the one count
 * (`sessionTasks.tasksAtWorkOf`), so the word agrees with the count on its row.
 */
export function useShownState({
  session,
  shell,
  report = null,
  outcome = null,
  harness,
}: ShownOf): Shown | undefined {
  const below = useTasksBelow(session);
  // Its question, where the asks registry says it stands (#4564): never a copy of its own.
  const question = useQuestionStanding(session);
  return useChatsSelect(
    useChatsHere(),
    (states) =>
      shownState({
        board: markOf(states, session, shell),
        needsYou: states.needsYou.includes(session) || question?.level === "needs-you",
        task:
          report === null
            ? null
            : { report, outcome, asking: question?.level === "with-asker" ? question.asker : null },
        harness,
        tasksAtWork: tasksAtWorkOf(states, below),
        ownWork: ownWorkOf(states, session),
      }),
    sameShown,
  );
}

/**
 * **Who a chat runs as, with its state on it** (#3342, the operator's ruling of 2026-10-26): the
 * persona's mark in the persona's colour, first on the chat's row. A chat that asks for the person
 * or is at work wears its state's own mark on the persona's corner, shape and colour (`App.css`
 * draws `StateShown`'s mark as that badge); a chat at rest wears none. A chat with no persona
 * wears the terminal's mark. Its tooltip and what a screen reader hears both say the two:
 * `steward, needs you`.
 *
 * **Its look is shared** (#4762): `chatPersona.css` holds it, and the place it stands sets only
 * its size (`--chat-persona-size`) and the ground its badge is ringed in (`--chat-badge-ground`).
 * The sidebar's row draws this component. A tab on the strip wears the same look with its own
 * marks (`ChatRows.TabMarks`): its persona's or its shell's, and the board state's dot. The word
 * it says after its name is this one's (`useShownState`, #4935).
 */
export const ChatPersonaState = memo(function ChatPersonaState({
  persona,
  session,
  shell,
  report = null,
  outcome = null,
  harness,
  changed = false,
}: ShownOf & {
  persona: string | null;
  changed?: boolean;
}) {
  const shown = useShownState({ session, shell, report, outcome, harness });
  const who = persona ?? "terminal";
  const said = shown === undefined ? who : `${who}, ${shown.word}`;
  return (
    <span className="chat-persona" data-state={shown?.kind}>
      {persona === null ? (
        <Tooltip tip={said}>
          <SquareTerminal className="node-icon" aria-hidden="true" />
        </Tooltip>
      ) : (
        <PersonaMark persona={persona} tip={said} />
      )}
      {/* Heard after the chat's name, as the persona and then the state: the state's word is
          `StateShown`'s, out of sight where it stands. */}
      <span className="hidden-words">{shown === undefined ? who : `${who},`}</span>
      {shown !== undefined && (
        <>
          {" "}
          <StateShown shown={shown} changed={changed} markOnly />
        </>
      )}
    </span>
  );
});
