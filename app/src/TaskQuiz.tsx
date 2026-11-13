// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState } from "react";
import { X } from "lucide-react";
import type { PlaneId, ReachedWhy, Shown } from "./bindings";
import { answerHowSaid, useAnswerQuestion } from "./AnswerQuestion";
import { ChatText } from "./ChatText";
import { CLOSE_ASK_SAYS, SHOW_LESS } from "./Notice";
import { KindMark, useNow } from "./InboxRow";
import { agoSaid } from "./inboxRules";
import { Count } from "./ui/badge/Badge";
import { Button, IconButton } from "./ui/button/Button";
import { Card } from "./ui/card/Card";
import { usePinnedRoom } from "./ui/notice/Notice";
import { Twist } from "./ui/disclosure/Disclosure";
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
export const moreSaidFully = (count: number) =>
  `${count} more ${count === 1 ? "question waits" : "questions wait"} after this`;
/** What the quiz's close says it does (#5213): the question is put away here, not answered. */
export const CLOSE_QUIZ = "Close question";
export const closeQuizSaid = CLOSE_ASK_SAYS;

/** A question's row (#5215): Answer opens it in the Inbox, and its toggle shows it whole. */
export const ANSWER_IN_INBOX = "Answer";
export const SHOW_WHOLE_QUESTION = "Show the whole question";

/** **A question's first line**, the first the task wrote with any words on it. */
export const firstLineOf = (says: string): string =>
  says.split("\n").find((line) => line.trim() !== "") ?? says;

/**
 * **A task question, drawn as a quiz in a pane's notices band** (#4565, spec #4551, the reviewed
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
 * - **A question and its note are a chat's text** (`ChatText`, #5214): over the lines they were
 *   written on (#5213), in markdown under the safe subset, as the Inbox draws them.
 * - **It is one row until it is opened** (#5215, the approved prototype): its mark, who asks and its
 *   first line, who it is a task of and its age, Answer (which opens it in the Inbox), a toggle and
 *   a ×. Opened, it shows whole, and the box to answer in is pinned at its foot.
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
  onAnswerInInbox,
  onAnswered,
  onTyped,
  onClose,
  defaultOpen = false,
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
  /** Opens the Inbox at this question: its row's Answer (#5215). */
  onAnswerInInbox?: (task: number) => void;
  /** The answer was taken. */
  onAnswered: () => void;
  /** What is in the box, as it changes: kept by the band, should an agent answer first. */
  onTyped?: (text: string) => void;
  /** Put away from this pane, unanswered. Without it the quiz has no close. */
  onClose?: () => void;
  /** Starts opened, whole: for a story or a test about what an opened question holds. */
  defaultOpen?: boolean;
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
  /** **Opened, it shows whole** (#5215): a row until then. What was typed is kept here either way. */
  const [open, setOpen] = useState(defaultOpen);
  const held = taken || sending || asked === undefined || asked === null;
  const why = ask.answer.via === "task-question" ? ask.answer : undefined;
  const age = ask.since == null ? undefined : agoSaid(ask.since * 1000, now);
  const toggleAt = useRef<HTMLButtonElement>(null);
  const pinnedAt = useRef<HTMLDivElement>(null);
  usePinnedRoom(pinnedAt, open);
  /** **Esc folds it** (#5215): what was typed is kept, and the keyboard lands on its toggle. */
  const fold = () => {
    setOpen(false);
    toggleAt.current?.focus();
  };
  return (
    <Card
      variant="ask"
      className="inbox-ask task-quiz"
      role="group"
      aria-label={who}
      data-testid="task-quiz"
      data-task={ask.session}
      data-whole={open ? "" : undefined}
      onKeyDown={(event) => {
        if (event.key !== "Escape" || !open) return;
        event.preventDefault();
        event.stopPropagation();
        fold();
      }}
    >
      <div className="task-quiz-row">
        <KindMark kind="reply" />
        <div className="task-quiz-words">
          <div className="task-quiz-title ui-text-body">
            <span className="inbox-who ui-text-strong">{open ? who : `${who}: `}</span>
            {!open && <ChatText className="task-quiz-first" whole text={firstLineOf(ask.says)} />}
          </div>
          <span className="task-quiz-sub ui-text-meta">
            {[why === undefined ? undefined : `A task of ${why.from}`, age]
              .filter((part) => part !== undefined)
              .join(" · ")}
          </span>
        </div>
        <span className="task-quiz-top-end">
          {more > 0 && (
            <Tooltip tip={moreSaidFully(more)}>
              <Count variant="neutral" said={moreSaidFully(more)}>
                {moreSaid(more)}
              </Count>
            </Tooltip>
          )}
          {onAnswerInInbox !== undefined && (
            <Button size="sm" variant="primary" onPress={() => onAnswerInInbox(ask.session)}>
              {ANSWER_IN_INBOX}
            </Button>
          )}
          <IconButton
            ref={toggleAt}
            size="sm"
            variant="ghost"
            aria-label={open ? SHOW_LESS : SHOW_WHOLE_QUESTION}
            aria-expanded={open}
            onPress={() => setOpen((was) => !was)}
            icon={<Twist open={open} />}
          />
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
      </div>
      {open && (
        <div className="task-quiz-whole">
          <ChatText id={`${ids}-asked`} className="inbox-primary" whole text={ask.says} />
          {why !== undefined && (
            <>
              <p className="task-quiz-why ui-text-secondary">{QUIZ_WHY[why.why](why.from)}</p>
              {why.note !== null && (
                <div className="task-quiz-note">
                  <span className="task-quiz-note-who ui-text-meta">{`${why.from}'s note`}</span>
                  <ChatText
                    className="task-quiz-note-words ui-text-secondary"
                    whole
                    text={why.note}
                  />
                </div>
              )}
            </>
          )}
          {/* **Its buttons stay in reach** (#5215): pinned at the message's foot as the band
              scrolls, above the band's fade. */}
          <div ref={pinnedAt} className="task-quiz-pinned">
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
                // Pinned, so it stays a few lines tall, and scrolls past them.
                rows={Math.min(Math.max(2, text.split("\n").length), 4)}
                value={text}
                isDisabled={held}
                onChange={(to) => {
                  setText(to);
                  setRefused(undefined);
                  onTyped?.(to);
                }}
                onKeyDown={(event) => {
                  // The field keeps its keys to itself, so its Esc is said here.
                  if (event.key === "Escape") {
                    event.preventDefault();
                    fold();
                    return;
                  }
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
          </div>
        </div>
      )}
    </Card>
  );
}
