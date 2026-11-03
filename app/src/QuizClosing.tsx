// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useState } from "react";
import { commands, type ChatAnsweredShown, type PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { Button } from "./ui/button/Button";
import { Form, TextArea } from "./ui/field/Field";

/** What the closing line says (the reviewed prototype's `copy.ts`, `closing`). */
export const closingSaid = (from: string, task: string, said: string) =>
  `${from} told ${task}: "${said}"`;
export const typedKeptSaid = (from: string, said: string) =>
  `${from} answered first: "${said}" What you typed is still in the box.`;
export const otherwiseHowSaid = (task: string, from: string) =>
  `${task} is told this replaces ${from}'s answer.`;
export const overriddenSaid = (task: string, from: string) =>
  `${task} was told your word replaces ${from}'s answer.`;
export const GO_TO_TASK = "Go to task";
export const SAY_OTHERWISE = "Say otherwise…";
export const OTHERWISE_BOX = "What you want instead";
export const SEND_AS_YOUR_WORD = "Send as your word";

/** A quiz an agent answered first, as the line it leaves holds it. */
export type Closed = {
  /** The task that asked, by number, and its question's number. */
  session: number;
  number: number;
  answered: ChatAnsweredShown;
  /** What the person had typed in the quiz when it closed, where they had typed anything. */
  typed?: string;
};

/**
 * **The line a quiz leaves when an agent answered it first** (#4566, spec #4551 story 36, the
 * reviewed prototype's closing line): the quiz closes quietly, with no error, and this says who
 * said what. **It stays until dismissed** (the person's ruling of 2026-11-02).
 *
 * - **Go to task** opens the task, as a press of the person's does.
 * - **Say otherwise…** opens a box under the line for the person's later word, which reaches the
 *   task as theirs, marked as overriding the chat's answer (story 37,
 *   `Talk::person_answers`). Once sent, the line says the task was told.
 * - **What the person had typed in the quiz is kept**: the box is open with it, to send as their
 *   word.
 *
 * Nothing here takes the keyboard by itself: only Say otherwise, the person's own press, moves it
 * to the box. The chat's words are data, drawn as text and masked by the core.
 */
export function QuizClosing({
  plane,
  closed,
  onGoToTask,
  onDismiss,
}: {
  plane: PlaneId;
  closed: Closed;
  onGoToTask: (task: number) => void;
  onDismiss: () => void;
}) {
  const { session, number, answered, typed } = closed;
  const { task, from, said, question } = answered;
  const kept = typed !== undefined && typed.trim() !== "";
  const [saying, setSaying] = useState(kept);
  const [text, setText] = useState(kept ? typed : "");
  const [sending, setSending] = useState(false);
  const [refused, setRefused] = useState<string>();
  const [overridden, setOverridden] = useState(false);
  const formId = useId();
  const cause = `task-question:closed:${session}:${number}`;

  if (overridden)
    return (
      <Notice at="pane" cause={cause} onDismiss={onDismiss}>
        {overriddenSaid(task, from)}
      </Notice>
    );

  const send = async () => {
    if (sending) return;
    if (text.trim() === "") {
      setRefused("Type what you want instead, then send it.");
      return;
    }
    setSending(true);
    setRefused(undefined);
    const answer = await commands
      .answerTaskQuestion(plane, session, number, question, text, true)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setSending(false);
    if (answer.status === "error") setRefused(answer.error);
    else setOverridden(true);
  };

  const form = saying ? (
    <Form
      id={formId}
      className="quiz-closing-form"
      aria-label={`Say otherwise to ${task}`}
      onSubmit={(event) => {
        event.preventDefault();
        void send();
      }}
    >
      <div className="task-quiz-answer">
        <TextArea
          className="task-quiz-box"
          aria-label={OTHERWISE_BOX}
          aria-describedby={`${formId}-how`}
          placeholder={OTHERWISE_BOX}
          // The person pressed Say otherwise: their press, so the box may take the keyboard.
          autoFocus={!kept}
          rows={Math.min(Math.max(2, text.split("\n").length), 10)}
          value={text}
          // The core's refusal is said on the line under the box, on the Notice's own ground.
          isInvalid={refused !== undefined}
          isDisabled={sending}
          onChange={(to) => {
            setText(to);
            setRefused(undefined);
          }}
        />
        <Button size="sm" variant="primary" type="submit" isDisabled={sending}>
          {kept ? SEND_AS_YOUR_WORD : "Send"}
        </Button>
        {!kept && (
          <Button size="sm" variant="ghost" onPress={() => setSaying(false)}>
            Cancel
          </Button>
        )}
      </div>
      <p className="ui-text-meta quiz-closing-how" id={`${formId}-how`}>
        {refused ?? otherwiseHowSaid(task, from)}
      </p>
    </Form>
  ) : undefined;

  const goToTask = { label: GO_TO_TASK, onPress: () => onGoToTask(session) };
  if (kept)
    return (
      <Notice at="pane" cause={cause} link={goToTask} onDismiss={onDismiss} under={form}>
        {typedKeptSaid(from, said)}
      </Notice>
    );
  return (
    <Notice
      at="pane"
      cause={cause}
      link={goToTask}
      fixes={[
        {
          label: SAY_OTHERWISE,
          onPress: () => setSaying((open) => !open),
          opens: { id: formId, open: saying },
        },
      ]}
      onDismiss={onDismiss}
      under={form}
    >
      {closingSaid(from, task, said)}
    </Notice>
  );
}
