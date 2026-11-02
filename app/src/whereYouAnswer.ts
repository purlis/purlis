// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Shown } from "./bindings";
import { layoutPref } from "./layoutPref";

/**
 * **Where you answer** (#4565, spec #4551, ADR 0107): where the person sees a task question that
 * has reached them. **From the top**, the default: one at a time, as a quiz at the top of the
 * pane of the nearest chat above it with a tab (`sessionOf`, as a task's session tab is defined).
 * **From each session**: in its task's own pane. It moves what the person sees, never who is
 * asked first, and nothing is rerouted when it changes: every surface reads it as it draws. Any
 * task can still be opened and answered in its own pane either way, and the question stays in the
 * ✋ and the Inbox wherever it is drawn.
 *
 * **Kept in the layout file** (`layout.json`, `regions.ts`), under `questions`, beside how the
 * Chats list is drawn: how one person likes to be asked, on this machine. `offered` is whether
 * the quiet line above the first question has been answered, so it is offered once.
 */
export type WhereYouAnswer = "top" | "each";

export type WhereYouAnswerPrefs = {
  answer: WhereYouAnswer;
  /** Whether the person has answered the line that offers the choice, or answered a quiz. */
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

/** A quiz a pane draws: the question, how many more wait at the same top, and whether it is the
 *  pane's own chat's question (drawn with no Open task). */
export type QuizHere = { ask: Shown; more: number; own: boolean };

/**
 * **Which task question the pane showing chat `shown` draws as a quiz**, if any (#4565).
 *
 * - **The chat's own question**, in either setting: any task can be opened and answered.
 * - **From the top**: the oldest question whose top (`topOf`, the nearest chat above it with a
 *   tab) is `shown`, with how many more wait there. One at a time, oldest first.
 * - Nothing else: **only a task question is drawn as a quiz**. Every other ask stays on its chat
 *   and in the Inbox, as does a question whose top has no tab.
 *
 * Plain inputs and no state: the asks are the registry's list, never a copy.
 */
export function quizIn(
  asks: readonly Shown[],
  shown: number,
  where: WhereYouAnswer,
  topOf: (task: number) => number | undefined,
): QuizHere | undefined {
  const questions = asks.filter((ask) => ask.source === "task-question");
  const own = questions.find((ask) => ask.session === shown);
  if (own !== undefined) return { ask: own, more: 0, own: true };
  if (where !== "top") return undefined;
  const here = questions
    .filter((ask) => topOf(ask.session) === shown)
    .sort((one, other) => (one.since ?? 0) - (other.since ?? 0) || one.session - other.session);
  if (here.length === 0) return undefined;
  return { ask: here[0], more: here.length - 1, own: false };
}
