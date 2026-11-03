// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo } from "react";
import { SquareTerminal } from "lucide-react";
import { markOf, ownWorkOf, useChatsHere, useChatsSelect } from "./chatState";
import { PersonaMark } from "./PersonaMark";
import { shownState, type Shown, type TaskFacts } from "./shownState";
import { StateShown } from "./StateShown";
import { tasksAtWorkOf } from "./sessionTasks";
import { useTasksBelow } from "./TasksBelow";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** Whether two shown states say the same thing. */
export function sameShown(one: Shown | undefined, other: Shown | undefined): boolean {
  return one === other || (one?.kind === other?.kind && one?.word === other?.word);
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
 * marks (`ChatRows.TabMarks`): its persona's or its shell's, and the board state's dot, so the
 * state it says after its name is the one its dot shows.
 */
export const ChatPersonaState = memo(function ChatPersonaState({
  persona,
  session,
  shell,
  report = null,
  outcome = null,
  asking = null,
  harness,
  changed = false,
}: {
  persona: string | null;
  session: number;
  changed?: boolean;
  shell: boolean;
  report?: TaskFacts["report"] | null;
  outcome?: string | null;
  asking?: string | null;
  harness: string | null;
}) {
  const below = useTasksBelow(session);
  const shown = useChatsSelect(
    useChatsHere(),
    (states) =>
      shownState({
        board: markOf(states, session, shell),
        needsYou: states.needsYou.includes(session),
        task: report === null ? null : { report, outcome, asking },
        harness,
        tasksAtWork: tasksAtWorkOf(states, below),
        ownWork: ownWorkOf(states, session),
      }),
    sameShown,
  );
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
