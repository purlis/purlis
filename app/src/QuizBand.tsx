// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { commands, type PlaneId, type Shown } from "./bindings";
import { Notice, useBandRows } from "./Notice";
import { useQuestionsHere } from "./QuestionsHere";
import { QuizClosing, type Closed } from "./QuizClosing";
import { closeQuizSaid, moreSaid, moreSaidFully, TaskQuiz } from "./TaskQuiz";
import { Count } from "./ui/badge/Badge";
import { Card } from "./ui/card/Card";
import { quizIn, setWhereYouAnswer, useWhereYouAnswer, type QuizHere } from "./whereYouAnswer";

/** What the note above the first question says, and its one way out (#5213, the prototype of
 *  the pane's notices band). Keeping the setting is the default, so it needs no answer. */
export const FIRST_LINE =
  "Questions from this chat's tasks now come here, so you answer them in one place.";
export const CHANGE = "Change";
/** What the folded band's line calls a task's question (#5215). */
export const QUESTION_WHAT = "A task's question";

/**
 * **Where the window places a task's question**, lent to every pane by the window (#4565):
 * `topOf` is a task's top, the chat it returns to, the nearest above it with a pane
 * (`returnsTo`), and `openTask` shows a task as a press of the person's does.
 */
export const QuizPlaces = createContext<
  | {
      topOf: (task: number) => number | undefined;
      openTask: (task: number) => void;
      /** Opens Settings at Where you answer, for the note's Change. */
      openSettings: () => void;
      /** Opens the Inbox at a task's question, for its row's Answer (#5215). */
      answerInInbox: (task: number) => void;
    }
  | undefined
>(undefined);

/** **The pause before the next quiz** (#4566, spec #4551 story 33): long enough that a second
 *  click meant for the quiz that went lands on nothing (the prototype measured about a second). */
export const PAUSE_MS = 1000;
export const sentSaid = (task: string) => `Sent to ${task}.`;
export const NEXT_SOON = "The next question comes in a moment.";
export const cannotDrawSaid = (task: string) =>
  `${task} asks something this tab cannot draw. Answer it in the task's pane.`;

/** The task a question is from, by the name the person sees: the chain's last. */
const taskOf = (ask: Shown) => ask.chain[ask.chain.length - 1] ?? `chat ${ask.session}`;

/** A task question's number, from its key in the asks registry (`task-question:<task>:<n>`,
 *  `asking::task_question`). */
function numberOf(ask: Shown): number | undefined {
  if (ask.source !== "task-question") return undefined;
  const number = Number(ask.ask.split(":")[2]);
  return Number.isInteger(number) ? number : undefined;
}

/**
 * **Which pane offers the choice of where you answer** (#4566, from #4938's review): with split
 * panes, two tops can each draw a quiz, and the quiet line is offered in one of them only, the
 * first that wanted it. Held by the window, outside any one pane.
 */
const offering = (() => {
  let wanting: string[] = [];
  const heard = new Set<() => void>();
  const told = () => heard.forEach((hear) => hear());
  return {
    want(id: string) {
      if (wanting.includes(id)) return;
      wanting = [...wanting, id];
      told();
    },
    drop(id: string) {
      if (!wanting.includes(id)) return;
      wanting = wanting.filter((one) => one !== id);
      told();
    },
    first: () => wanting[0],
    listen(hear: () => void) {
      heard.add(hear);
      return () => void heard.delete(hear);
    },
  };
})();

/** Whether this pane, wanting to offer the choice, is the one that does. */
function useOffersHere(wants: boolean): boolean {
  const id = useId();
  useEffect(() => {
    if (!wants) return;
    offering.want(id);
    return () => offering.drop(id);
  }, [wants, id]);
  const first = useSyncExternalStore(offering.listen, offering.first);
  return wants && first === id;
}

/**
 * **The quiz a pane draws, held across the registry's lists** (#4566): one at a time, and never
 * swapped under the person's pointer. When the quiz drawn goes and another takes its place, the
 * band pauses {@link PAUSE_MS} at the quiz's own height first, saying where the answer went
 * where it was theirs. A quiz that went because an asking chat answered it first leaves its
 * closing line ({@link QuizClosing}), which the core says who gave, and nothing where the person
 * answered it elsewhere: one answer clears every surface, with no error.
 */
function useQuizzes(
  plane: PlaneId,
  quiz: QuizHere | undefined,
  asks: readonly Shown[],
  pauseMs: number,
) {
  const [pause, setPause] = useState<{ sent?: string; height: number }>();
  const [closings, setClosings] = useState<Closed[]>([]);
  /** The quiz drawn at the last commit, its height, and what the person had typed in it. */
  const drawn = useRef<Shown>(undefined);
  const height = useRef(0);
  const typed = useRef(new Map<string, string>());
  /** The questions the person answered in this band. */
  const sent = useRef(new Set<string>());
  /** The quiz that went last before the person's answer to it was taken, as the registry's
   *  next list may reach the window before the answer's own reply does. */
  const gone = useRef<{ ask: string; task: string }>(undefined);
  const box = useRef<HTMLDivElement>(null);
  const key = quiz?.ask.ask;

  useLayoutEffect(() => {
    const before = drawn.current;
    drawn.current = quiz?.ask;
    if (before === undefined || before.ask === key) return;
    const ours = sent.current.has(before.ask);
    const left = !asks.some((ask) => ask.ask === before.ask);
    const kept = typed.current.get(before.ask);
    typed.current.delete(before.ask);
    const number = numberOf(before);
    gone.current = left && !ours ? { ask: before.ask, task: taskOf(before) } : undefined;
    if (left && !ours && number !== undefined) {
      void commands
        .taskQuestionAnswered(plane, before.session, number)
        .then((answer) => {
          if (answer.status !== "ok" || answer.data === null) return;
          // Their own answer was taken after all: there is nothing they did not see.
          if (sent.current.has(before.ask)) return;
          const answered = answer.data;
          setClosings((now) =>
            now.some((one) => one.session === before.session && one.number === number)
              ? now
              : [
                  ...now,
                  {
                    session: before.session,
                    number,
                    answered,
                    ...(kept === undefined ? {} : { typed: kept }),
                  },
                ],
          );
        })
        .catch(() => {});
    }
    // Another quiz takes this one's place: never under the pointer.
    if (key !== undefined)
      setPause({ ...(ours && left ? { sent: taskOf(before) } : {}), height: height.current });
    // Read only as the quiz drawn changes: the list is what it was at that commit.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  // The height the quiz stands at, for the pause to keep (after the effect above, which reads
  // the last commit's).
  useLayoutEffect(() => {
    if (box.current !== null) height.current = box.current.offsetHeight;
  });

  useEffect(() => {
    if (pause === undefined) return;
    const over = setTimeout(() => setPause(undefined), pauseMs);
    return () => clearTimeout(over);
  }, [pause, pauseMs]);

  return {
    pausing: pause !== undefined && quiz !== undefined ? pause : undefined,
    closings,
    dismiss: (closed: Closed) => setClosings((now) => now.filter((one) => one !== closed)),
    box,
    answered: (ask: string) => {
      sent.current.add(ask);
      const went = gone.current;
      if (went?.ask !== ask) return;
      gone.current = undefined;
      setPause((now) => (now === undefined ? now : { ...now, sent: went.task }));
    },
    typedIn: (ask: string, text: string) => typed.current.set(ask, text),
  };
}

/**
 * **A task's question in a pane** (#4565, #4566): the quiz stands above the terminal, because a
 * chat's input is inside it and a quiz at its foot moved the input line.
 *
 * The pane showing chat `session` draws the quiz {@link quizIn} picks: the chat's own question,
 * and from the top the oldest that waits at it, with a count of the rest. **One at a time**, with
 * a short pause before the next ({@link useQuizzes}), and the line an agent's answer leaves
 * above it. A task's question the window cannot draw is a line linking to its pane, in the same
 * place in the queue. Above the first one that reaches the person, **a note says once** where
 * they come, never a dialog, and in one pane only, with Change, which opens Settings at Where you
 * answer. Answering the quiz counts as keeping the setting.
 * The setting is read as it is drawn, so a change applies to every open question at once and
 * reroutes nothing.
 *
 * **It is part of the pane's notices band, under the pane's head** (#5213, #4754): each of its
 * parts is one box of the band's stack, so the band folds them by its one setting. Its notes
 * (the line about where questions come, an agent's answer) fold into the bell while the band is
 * minimised, and the quiz stands, as every ask does (`data-waits`). Everything in it closes: the
 * note for good, by its Dismiss, which counts as keeping the setting, and a question from this
 * pane only, drawn or not, so it stays in the Inbox and on the ✋ until it is answered.
 */
export function QuizBand({
  plane,
  session,
  pauseMs = PAUSE_MS,
}: {
  plane: PlaneId;
  session: number;
  /** How long the pause before the next quiz lasts: {@link PAUSE_MS}, but for a story that
   *  pictures the pause, which a second is too short to catch every time. */
  pauseMs?: number;
}) {
  const held = useQuestionsHere();
  const places = useContext(QuizPlaces);
  const prefs = useWhereYouAnswer();
  /** The questions closed in this pane, kept with its band (#5215): put away here, still the
   *  registry's, so still the Inbox's and the ✋'s. */
  const rows = useBandRows();
  const asks = held?.asks ?? [];
  const open = asks.filter((ask) => rows?.closed.has(ask.ask) !== true);
  const quiz = places === undefined ? undefined : quizIn(open, session, prefs.answer, places.topOf);
  const { pausing, closings, dismiss, box, answered, typedIn } = useQuizzes(
    plane,
    quiz,
    asks,
    pauseMs,
  );
  const offers = useOffersHere(
    quiz !== undefined && quiz.drawn && !quiz.own && !prefs.offered && prefs.answer === "top",
  );
  if (quiz === undefined && closings.length === 0) return null;
  const closeQuiz = () => {
    if (quiz !== undefined) rows?.close(quiz.ask.ask);
  };
  return (
    <>
      {closings.map((closing) => (
        <QuizClosing
          key={`${closing.session}:${closing.number}`}
          plane={plane}
          closed={closing}
          onGoToTask={(task) => places?.openTask(task)}
          onDismiss={() => dismiss(closing)}
        />
      ))}
      {pausing !== undefined && (
        // The quiz's own height, measured as it went, so nothing under the band moves.
        <div
          className="quiz-band-quiz"
          data-waits=""
          data-what={QUESTION_WHAT}
          style={{ minBlockSize: `${pausing.height}px` }}
        >
          <Card variant="ask" className="inbox-ask task-quiz task-quiz-pause">
            {pausing.sent !== undefined && <p className="ui-text-body">{sentSaid(pausing.sent)}</p>}
            <p className="ui-text-secondary">{NEXT_SOON}</p>
          </Card>
        </div>
      )}
      {pausing === undefined && offers && (
        <Notice
          at="pane"
          cause="where-you-answer:offered"
          link={{ label: CHANGE, onPress: () => places?.openSettings() }}
          onDismiss={() => setWhereYouAnswer({ offered: true })}
          dismissSays="Keeps questions here, and never says this again."
        >
          {FIRST_LINE}
        </Notice>
      )}
      {pausing === undefined && quiz !== undefined && places !== undefined && (
        <div ref={box} className="quiz-band-quiz" data-waits="" data-what={QUESTION_WHAT}>
          {quiz.drawn ? (
            <TaskQuiz
              // A new question is a new quiz: nothing typed for one is carried to the next.
              key={quiz.ask.ask}
              plane={plane}
              ask={quiz.ask}
              more={quiz.more}
              own={quiz.own}
              onOpenTask={places.openTask}
              onAnswerInInbox={places.answerInInbox}
              onAnswered={() => {
                answered(quiz.ask.ask);
                if (!prefs.offered) setWhereYouAnswer({ offered: true });
              }}
              onTyped={(text) => typedIn(quiz.ask.ask, text)}
              onClose={closeQuiz}
            />
          ) : (
            <Notice
              at="pane"
              tone="attention"
              cause={`task-question:cannot-draw:${quiz.ask.session}`}
              link={{ label: "Open task", onPress: () => places.openTask(quiz.ask.session) }}
              onDismiss={closeQuiz}
              dismissSays={closeQuizSaid}
            >
              {cannotDrawSaid(taskOf(quiz.ask))}
              {quiz.more > 0 && (
                <>
                  {" "}
                  <Count variant="neutral" said={moreSaidFully(quiz.more)}>
                    {moreSaid(quiz.more)}
                  </Count>
                </>
              )}
            </Notice>
          )}
        </div>
      )}
    </>
  );
}
