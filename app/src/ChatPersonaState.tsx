// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo, type ReactNode } from "react";
import { SquareTerminal } from "lucide-react";
import {
  asksThePerson,
  failedUnseen,
  failuresUnseenOf,
  markOf,
  ownWorkOf,
  useChatsHere,
  useChatsSelect,
  type ChatStates,
} from "./chatState";
import { PersonaMark } from "./PersonaMark";
import { shownState, urgentOf, type RowFacts, type Shown } from "./shownState";
import { useQuestionStanding, useQuestionsHere } from "./QuestionsHere";
import { NONE_BELOW, tasksAtWorkOf, type TasksBelow } from "./sessionTasks";
import { standingOf, type QuestionStanding } from "./taskQuestions";
import { useEveryTasksBelow, useTasksBelow } from "./TasksBelow";
import { ChatMark } from "./ui/chat-mark/ChatMark";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** Whether two shown states say the same thing. */
export function sameShown(one: Shown | undefined, other: Shown | undefined): boolean {
  return (
    one === other ||
    (one?.kind === other?.kind && one?.word === other?.word && one?.mark === other?.mark)
  );
}

/** What a chat's shown state is read from beside the board: its row's facts (`RowFacts`), of
 *  which its question is read here, from the asks registry (#4564). A chat that is no task
 *  leaves `report` and `outcome` out. */
export type ShownOf = {
  session: number;
  /** A shell tab, which shows no state until something reports one. */
  shell: boolean;
} & Partial<Pick<RowFacts, "report" | "outcome">> &
  Pick<RowFacts, "harness">;

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
    (states) => shownFrom(states, below, question, { session, shell, report, outcome, harness }),
    sameShown,
  );
}

/** A chat's shown state from the chats' states, its tasks below and its question: the one
 *  reading {@link useShownState} and {@link useUrgentChat} both make. */
function shownFrom(
  states: ChatStates,
  below: TasksBelow,
  question: QuestionStanding | undefined,
  { session, shell, report = null, outcome = null, harness }: ShownOf,
): Shown | undefined {
  return shownState({
    board: markOf(states, session, shell),
    needsYou: asksThePerson(states, session) || question?.level === "needs-you",
    failuresUnseen: failuresUnseenOf(states, session),
    failedUnseen: failedUnseen(states, session),
    task:
      report === null
        ? null
        : { report, outcome, asking: question?.level === "with-asker" ? question.asker : null },
    harness,
    tasksAtWork: tasksAtWorkOf(states, below),
    ownWork: ownWorkOf(states, session),
  });
}

/**
 * **The most urgent of `chats`** (#5342, spec #5339): the one whose state comes first in the one
 * order (`shownState.urgentOf`), each read as its own mark reads it, so a split tab wears the
 * mark of the pane that needs the person, never only the focused one's. Nothing where none of
 * them has a state. Drawn again only when which chat it is changes.
 */
export function useUrgentChat(chats: readonly ShownOf[]): number | undefined {
  const below = useEveryTasksBelow();
  const questions = useQuestionsHere();
  return useChatsSelect(
    useChatsHere(),
    (states) =>
      urgentOf(
        chats,
        (one) =>
          shownFrom(
            states,
            below.get(one.session) ?? NONE_BELOW,
            standingOf(questions, one.session),
            one,
          )?.mark,
      )?.session,
  );
}

/**
 * **A chat's mark, read off the chat** (#3342, #5340): who it runs as, with its one state on the
 * corner, drawn by the one family (`ui/chat-mark`) wherever a chat is drawn: its row in the list,
 * its tab, its pane's head and a folded pane's rail. They read one function (`useShownState`), so
 * none can say something the others do not. A chat with no persona wears the terminal's mark. Its
 * tooltip and what a screen reader hears both say the two: `steward, needs you`.
 *
 * **The place it stands sets only its size and the ground its badge is ringed in**
 * (`--chat-mark-size`, `--chat-mark-ground`). `who` stands for the mark of someone else's kind: a
 * task's own tab wears the task's (#1489). `heard={false}` is for a place that says the word after
 * its name itself: a tab (`TabMarks`).
 */
export const ChatPersonaState = memo(function ChatPersonaState({
  persona,
  session,
  shell,
  report = null,
  outcome = null,
  harness,
  changed = false,
  who: standIn,
  heard = true,
  grip,
}: ShownOf & {
  persona: string | null;
  changed?: boolean;
  who?: ReactNode;
  heard?: boolean;
  /** **The grip that stands in its `who`** (#5290, `ui/grip`): what its row or its pane is picked
   *  up by, handed the mark it turns into dots in place of. The badge stays beside it. */
  grip?: (mark: ReactNode) => ReactNode;
}) {
  const shown = useShownState({ session, shell, report, outcome, harness });
  const who = persona ?? "terminal";
  const said = shown === undefined ? who : `${who}, ${shown.word}`;
  const drawn =
    standIn ??
    (persona === null ? (
      <Tooltip tip={said}>
        <SquareTerminal
          className="node-icon"
          data-mark={shell ? "shell" : undefined}
          aria-hidden="true"
        />
      </Tooltip>
    ) : (
      <PersonaMark persona={persona} tip={said} />
    ));
  return (
    <ChatMark
      state={shown?.mark}
      changed={changed}
      who={
        <>
          {grip === undefined ? drawn : grip(drawn)}
          {/* Heard after the chat's name, as the persona and then the state. */}
          {heard && <span className="hidden-words">{shown === undefined ? who : `${who},`}</span>}
        </>
      }
      word={heard ? shown?.word : undefined}
    />
  );
});
