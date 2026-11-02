// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext, useState } from "react";
import type { PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { useQuestionsHere } from "./QuestionsHere";
import { TaskQuiz } from "./TaskQuiz";
import { quizIn, setWhereYouAnswer, useWhereYouAnswer } from "./whereYouAnswer";

/** What the line above the first question says, and its two answers (the prototype's words). */
export const FIRST_LINE = "Questions from tasks come to this tab, so you answer them in one place.";
export const ANSWER_FROM_EACH = "Answer from each session";
export const KEEP_THIS = "Keep this";
export const MOVED_LINE = "Questions now wait in each task's tab. Settings changes it back.";

/**
 * **Where the window places a task's question**, lent to every pane by the window (#4565):
 * `topOf` is a task's top, the nearest chat above it with a tab (`sessionOf`), and `openTask`
 * shows a task as a press of the person's does.
 */
export const QuizPlaces = createContext<
  | {
      topOf: (task: number) => number | undefined;
      openTask: (task: number) => void;
      /** Opens Settings at Where you answer, where the line that moved them says so. */
      openSettings: () => void;
    }
  | undefined
>(undefined);

/**
 * **The top of a pane, where a task's question is drawn** (#4565, the reviewed prototype's Band:
 * the quiz sits at the top of the pane, because a chat's input is inside its terminal and a quiz
 * at its foot moved the input line).
 *
 * The pane showing chat `session` draws the quiz {@link quizIn} picks: the chat's own question,
 * and from the top the oldest that waits at it, with a count of the rest. Above the first one
 * that reaches the person, **the choice is offered once** as a quiet line, never a dialog: Keep
 * this, or Answer from each session, which says where they went and how to change it back.
 * Answering the quiz counts as keeping it. The setting is read as it is drawn, so a change
 * applies to every open question at once and reroutes nothing.
 */
export function QuizBand({ plane, session }: { plane: PlaneId; session: number }) {
  const held = useQuestionsHere();
  const places = useContext(QuizPlaces);
  const prefs = useWhereYouAnswer();
  const [moved, setMoved] = useState(false);
  const quiz =
    places === undefined
      ? undefined
      : quizIn(held?.asks ?? [], session, prefs.answer, places.topOf);
  const offering = quiz !== undefined && !quiz.own && !prefs.offered && prefs.answer === "top";
  if (quiz === undefined && !moved) return null;
  return (
    <div className="quiz-band">
      {moved && (
        <Notice
          at="pane"
          cause="where-you-answer:moved"
          onDismiss={() => setMoved(false)}
          link={{ label: "Open settings", onPress: () => places?.openSettings() }}
        >
          {MOVED_LINE}
        </Notice>
      )}
      {offering && (
        <Notice
          at="pane"
          cause="where-you-answer:offered"
          fixes={[
            {
              label: ANSWER_FROM_EACH,
              onPress: () => {
                setWhereYouAnswer({ answer: "each", offered: true });
                setMoved(true);
              },
            },
            { label: KEEP_THIS, onPress: () => setWhereYouAnswer({ offered: true }) },
          ]}
        >
          {FIRST_LINE}
        </Notice>
      )}
      {quiz !== undefined && places !== undefined && (
        <TaskQuiz
          // A new question is a new quiz: nothing typed for one is carried to the next.
          key={quiz.ask.ask}
          plane={plane}
          ask={quiz.ask}
          more={quiz.more}
          own={quiz.own}
          onOpenTask={places.openTask}
          onAnswered={() => {
            if (!prefs.offered) setWhereYouAnswer({ offered: true });
          }}
        />
      )}
    </div>
  );
}
