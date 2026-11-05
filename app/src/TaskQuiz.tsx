// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { X } from "lucide-react";
import type { PlaneId, ReachedWhy, Shown } from "./bindings";
import { answerHowSaid, useAnswerQuestion } from "./AnswerQuestion";
import { ChatText } from "./ChatText";
import { KindMark, useNow } from "./InboxRow";
import { agoSaid } from "./inboxRules";
import { Count } from "./ui/badge/Badge";
import { Button, IconButton } from "./ui/button/Button";
import { Card } from "./ui/card/Card";
import { Form, TextArea } from "./ui/field/Field";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** Why a question reached the person, in the quiz's words (the prototype's `copy.ts`). */
export const QUIZ_WHY: Record<ReachedWhy, (chat: string) => string> = {
  "passed-on": (chat) => `${chat} passed this on.`,
  "no-reply": (chat) => `${chat} did not reply in 100 seconds.`,
  "asker-gone": (chat) => `${chat} has closed, so this came to you.`,
};

/** How many more wait behind the quiz, short and in full. */
export const moreSaid = (count: number) => `${count} more`;
/** What the quiz's close says it does (#5213): the question is put away here, not answered. */
export const CLOSE_QUIZ = "Close";
export const closeQuizSaid = "Close here. It stays in the Inbox until you answer it.";
export const moreSaidFully = (count: number) =>
  `${count} more ${count === 1 ? "question waits" : "questions wait"} after this`;

/**
 * **A task question, drawn as a quiz at the top of a pane** (#4565, spec #4551, the reviewed
 * prototype on `prototype/task-question-surfaces`): who asks, in the task's own words, why it
 * reached the person, the asking chat's note marked as its own, how long it has waited, a box for
 * the answer and, away from the task's own pane, Open task.
 *
 * - **It never takes the keyboard** (#4551 story 39): it arrives by itself, so the box waits to be
 *   pressed into and what the person was typing carries on where it was (`takesFocus: false`).
 *   Only a press of the person's moves anything, Open task included.
 * - **It is one item everywhere**: it reads the asks registry's item, the same the ✋ and the
 *   Inbox read, and the answer goes the one path a task's question is answered by
 *   (`answer_task_question`). One answer clears it from every surface.
 * - **A question is a chat's text** (`ChatText`, #5214): over the lines its task wrote (#5213), in
 *   markdown under the safe subset, as the Inbox draws it.
 * - **It closes** (#5213): Close puts the quiz away from this pane and answers nothing, so the
 *   question stays in the Inbox and on the ✋ until it is answered.
 */
export function TaskQuiz({
  plane,
  ask,
  more,
  own,
  now: given,
  onOpenTask,
  onAnswered,
  onTyped,
  onClose,
}: {
  plane: PlaneId;
  /** The question, as the asks registry lists it (`source: "task-question"`). */
  ask: Shown;
  /** How many more questions wait at the same top, behind this one. */
  more: number;
  /** Whether the pane shows the task itself, which has no Open task. */
  own: boolean;
  /** The time to age it by, for a story or a test: otherwise read each minute. */
  now?: number;
  onOpenTask: (task: number) => void;
  /** The answer was taken. */
  onAnswered: () => void;
  /** What is in the box, as it changes: kept by the band, should an agent answer first. */
  onTyped?: (text: string) => void;
  /** Put away from this pane, unanswered. Without it the quiz has no close. */
  onClose?: () => void;
}) {
  const read = useNow();
  const now = given ?? read;
  const task = ask.chain[ask.chain.length - 1] ?? `chat ${ask.session}`;
  const who = ask.chain.length > 2 ? ask.chain.join(" › ") : `${task} asks`;
  /** The answer was taken: the box is emptied and held until the registry's next list takes
   *  the quiz away, so a second Enter sends nothing (#4566). */
  const [taken, setTaken] = useState(false);
  const answering = useAnswerQuestion({
    plane,
    session: ask.session,
    task,
    onDone: () => {
      setTaken(true);
      setText("");
      onTyped?.("");
      onAnswered();
    },
    takesFocus: false,
  });
  const { asked, text, setText, sending, refused, setRefused, send, box, ids } = answering;
  const held = taken || sending || asked === undefined || asked === null;
  const why = ask.answer.via === "task-question" ? ask.answer : undefined;
  const age = ask.since == null ? undefined : agoSaid(ask.since * 1000, now);
  return (
    <Card
      variant="ask"
      className="inbox-ask task-quiz"
      role="group"
      aria-label={who}
      data-testid="task-quiz"
      data-task={ask.session}
    >
      <p className="inbox-ask-top">
        <span className="inbox-who ui-text-body ui-text-strong">{who}</span>
        <span className="task-quiz-top-end">
          {more > 0 && (
            <Tooltip tip={moreSaidFully(more)}>
              <Count variant="neutral" said={moreSaidFully(more)}>
                {moreSaid(more)}
              </Count>
            </Tooltip>
          )}
          {age !== undefined && <span className="ui-text-meta">{age}</span>}
          {onClose !== undefined && (
            <IconButton
              size="sm"
              variant="ghost"
              aria-label={CLOSE_QUIZ}
              tooltip={closeQuizSaid}
              onPress={onClose}
              icon={<X aria-hidden="true" />}
            />
          )}
        </span>
      </p>
      <div className="inbox-says">
        <KindMark kind="reply" />
        <ChatText id={`${ids}-asked`} className="inbox-primary" text={ask.says} />
      </div>
      {why !== undefined && (
        <>
          <p className="task-quiz-why ui-text-secondary">{QUIZ_WHY[why.why](why.from)}</p>
          {why.note !== null && (
            <p className="task-quiz-note">
              <span className="task-quiz-note-who ui-text-meta">{`${why.from}'s note`}</span>
              <span className="ui-text-secondary">{why.note}</span>
            </p>
          )}
        </>
      )}
      <Form
        className="task-quiz-answer"
        aria-label={`Answer ${task}'s question`}
        onSubmit={(event) => {
          event.preventDefault();
          if (!taken) void send();
        }}
      >
        <TextArea
          inputRef={box}
          className="task-quiz-box"
          aria-label="Your answer"
          aria-describedby={`${ids}-asked ${ids}-how`}
          placeholder="Your answer"
          error={refused}
          rows={Math.min(Math.max(2, text.split("\n").length), 10)}
          value={text}
          isDisabled={held}
          onChange={(to) => {
            setText(to);
            setRefused(undefined);
            onTyped?.(to);
          }}
          onKeyDown={(event) => {
            if (event.key !== "Enter" || event.nativeEvent.isComposing) return;
            if (event.shiftKey || event.altKey) return;
            event.preventDefault();
            if (!taken) void send();
          }}
        />
        <Button size="sm" variant="primary" type="submit" isDisabled={held}>
          Send
        </Button>
      </Form>
      <div className="task-quiz-foot">
        <span className="ui-text-meta" id={`${ids}-how`}>
          {answerHowSaid(task)}
        </span>
        {!own && (
          <Button size="sm" variant="ghost" onPress={() => onOpenTask(ask.session)}>
            Open task
          </Button>
        )}
      </div>
    </Card>
  );
}
