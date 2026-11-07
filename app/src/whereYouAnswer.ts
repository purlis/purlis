// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Shown } from "./bindings";
import { layoutPref } from "./layoutPref";

/**
 * **Where you answer** (#4565, spec #4551, ADR 0107): where the person sees a task question that
 * has reached them. **From the top**, the default: one at a time, as a quiz in the notices band of
 * the pane of the nearest chat above it with a pane, the one it returns to (`returnsTo`).
 * **From each session**: in its task's own pane. It moves what the person sees, never who is
 * asked first, and nothing is rerouted when it changes: every surface reads it as it draws. Any
 * task can still be opened and answered in its own pane either way, and the question stays in the
 * ✋ and the Inbox wherever it is drawn.
 *
 * **Kept in the layout file** (`layout.json`, `regions.ts`), under `questions`, beside how the
 * Chats list is drawn: how one person likes to be asked, on this machine. `offered` is whether
 * the note above the first question has been put away, so it shows once.
 */
export type WhereYouAnswer = "top" | "each";

export type WhereYouAnswerPrefs = {
  answer: WhereYouAnswer;
  /** Whether the person has put away the note about where questions come, or answered a quiz. */
  offered: boolean;
};

export const DEFAULT_WHERE_YOU_ANSWER: WhereYouAnswerPrefs = { answer: "top", offered: false };

const isObject = (raw: unknown): raw is Record<string, unknown> =>
  raw !== null && typeof raw === "object" && !Array.isArray(raw);

/** The preference as a layout document holds it, and what had to be put right. */
export function loadWhereYouAnswer(raw: unknown): { prefs: WhereYouAnswerPrefs; said: string[] } {
  const said: string[] = [];
  const held = isObject(raw) ? raw.questions : undefined;
  if (held === undefined) return { prefs: DEFAULT_WHERE_YOU_ANSWER, said };
  if (!isObject(held)) {
    said.push(`"questions" ${JSON.stringify(held)} is not where you answer, so it is from the top`);
    return { prefs: DEFAULT_WHERE_YOU_ANSWER, said };
  }
  let answer = DEFAULT_WHERE_YOU_ANSWER.answer;
  if (held.answer === "top" || held.answer === "each") answer = held.answer;
  else if (held.answer !== undefined)
    said.push(
      `"questions.answer" ${JSON.stringify(held.answer)} is not "top" or "each", so it is "top"`,
    );
  let offered = DEFAULT_WHERE_YOU_ANSWER.offered;
  if (typeof held.offered === "boolean") offered = held.offered;
  else if (held.offered !== undefined)
    said.push(
      `"questions.offered" ${JSON.stringify(held.offered)} is not true or false, so it is false`,
    );
  return { prefs: { answer, offered }, said };
}

/** What the file keeps: only the fields off their default, or nothing. */
function writtenOf(prefs: WhereYouAnswerPrefs): Partial<WhereYouAnswerPrefs> | undefined {
  const kept: Partial<WhereYouAnswerPrefs> = {};
  if (prefs.answer !== DEFAULT_WHERE_YOU_ANSWER.answer) kept.answer = prefs.answer;
  if (prefs.offered !== DEFAULT_WHERE_YOU_ANSWER.offered) kept.offered = prefs.offered;
  return Object.keys(kept).length === 0 ? undefined : kept;
}

/** The store (`layoutPref.ts`), under `questions`. */
export const WHERE_YOU_ANSWER = layoutPref<WhereYouAnswerPrefs, "questions">({
  key: "questions",
  fallback: DEFAULT_WHERE_YOU_ANSWER,
  load: (raw) => {
    const { prefs, said } = loadWhereYouAnswer(raw);
    return { value: prefs, said };
  },
  same: (one, other) => one.answer === other.answer && one.offered === other.offered,
  written: writtenOf,
  remedy: (where) => `fix ${where}, or change Where you answer in Settings, which rewrites it`,
  settings: "you.chats",
});

/** Where the person answers now. */
export function whereYouAnswer(): WhereYouAnswerPrefs {
  return WHERE_YOU_ANSWER.value();
}

/** Changes it; tells every listener, which writes the file, when it changed. */
export function setWhereYouAnswer(to: Partial<WhereYouAnswerPrefs>): void {
  WHERE_YOU_ANSWER.set({ ...whereYouAnswer(), ...to });
}

/** {@link whereYouAnswer}, for a component that redraws when it changes. */
export function useWhereYouAnswer(): WhereYouAnswerPrefs {
  return WHERE_YOU_ANSWER.use();
}

/** A quiz a pane draws: the question, how many more wait at the same top, whether it is the
 *  pane's own chat's question (drawn with no Open task), and whether the window can draw it. */
export type QuizHere = { ask: Shown; more: number; own: boolean; drawn: boolean };

/** Whether `ask` is a question the window draws as a quiz: a task question. */
const isQuiz = (ask: Shown) => ask.source === "task-question";

/**
 * Whether `ask` is **a task's question the window cannot draw** (#4566, spec #4551 story 62): a
 * prompt in a task's own terminal, which purlis holds nothing to answer by (a harness with no
 * adapter, or a redirect that failed). It takes the same route, linked from the top to the
 * task's pane, where it is answered.
 */
const isUndrawable = (ask: Shown, topOf: (task: number) => number | undefined) =>
  ask.source === "terminal" && topOf(ask.session) !== undefined;

/** Oldest first; one whose age is unknown after every one whose age is known. */
const oldestFirst = (one: Shown, other: Shown) =>
  (one.since ?? Number.POSITIVE_INFINITY) - (other.since ?? Number.POSITIVE_INFINITY) ||
  one.session - other.session;

/**
 * **Which task question the pane showing chat `shown` draws as a quiz**, if any (#4565, #4566).
 *
 * - **The chat's own question**, in either setting: any task can be opened and answered. From
 *   the top, the questions waiting at it behind its own are counted in `more`.
 * - **From the top**: the oldest question whose top (`topOf`, the nearest chat above it with a
 *   tab) is `shown`, with how many more wait there. One at a time, oldest first.
 * - **A task's question the window cannot draw** takes the same route ({@link isUndrawable}),
 *   `drawn: false`: linked from the top, and left to its terminal and the waiting strip in its
 *   own pane.
 * - Nothing else: every other ask stays on its chat and in the Inbox, as does a question whose
 *   top has no tab.
 *
 * Plain inputs and no state: the asks are the registry's list, never a copy.
 */
export function quizIn(
  asks: readonly Shown[],
  shown: number,
  where: WhereYouAnswer,
  topOf: (task: number) => number | undefined,
): QuizHere | undefined {
  const waitingHere =
    where === "top"
      ? asks
          .filter((ask) => isQuiz(ask) || isUndrawable(ask, topOf))
          .filter((ask) => ask.session !== shown && topOf(ask.session) === shown)
          .sort(oldestFirst)
      : [];
  const own = asks.find((ask) => isQuiz(ask) && ask.session === shown);
  if (own !== undefined) return { ask: own, more: waitingHere.length, own: true, drawn: true };
  if (waitingHere.length === 0) return undefined;
  const first = waitingHere[0];
  return { ask: first, more: waitingHere.length - 1, own: false, drawn: isQuiz(first) };
}
