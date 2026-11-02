// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A task question's trail, in the window's words** (#4567, spec #4551, ADR 0107): each step the
 * core recorded of a question (`question_trail`), said as the reviewed prototype says it (#4561).
 *
 * **It holds no question words** (ADR 0075): what it is handed is a closed set of steps, and every
 * word it says is one of these, with the task's and its asking chat's names.
 */
import type { QuestionStep } from "./bindings";
import { saidAt } from "./dispatches";

/** The trail's own words, as the prototype has them (`prototype-task-questions/copy.ts`). */
export const TRAIL = {
  open: "What happened",
  title: "What happened to this question",
  words: "The trail holds who did what and when, never the question's words.",
  none: "purlis has no record of what happened to this question. A question asked before this version was not recorded.",
  reading: "Reading what happened…",
  unread: (why: string) => `purlis could not read what happened to this question: ${why}`,
  asked: (task: string) => `${task} asked`,
  handed: (chat: string) => `Handed to ${chat}`,
  passed: (chat: string) => `${chat} passed it on`,
  passedNote: (chat: string) => `${chat} passed it on, with a note`,
  noReply: (chat: string) => `${chat} did not reply in 100 seconds`,
  gone: (chat: string) => `${chat} closed`,
  reached: "Reached you",
  chatAnswered: (chat: string) => `${chat} answered`,
  youAnswered: "You answered",
  taskEnded: "The task ended, unanswered",
  redirectFailed: "purlis could not hand it on",
  notReadBack: "purlis restarted, and the question was not kept",
} as const;

/** One line of a trail: when, as the trail says it, and what. */
export type TrailLine = { at: string; time: string; said: string };

/** `hh:mm` of a step on the day the question was asked, else its day too, as Activity says it. */
function timeOf(at: string, day: string | undefined): string {
  return at.slice(0, 10) === day ? at.slice(11, 16) : saidAt(at);
}

/**
 * **The lines of the trail of a question `task` asked of its asking chat, `asker`**, from the
 * core's `steps`, oldest first. A step that says nothing the line before it did not (a question
 * closed by the answer just said) has no line of its own.
 */
export function trailSaid(
  steps: readonly QuestionStep[],
  task: string,
  asker: string,
): TrailLine[] {
  const day = steps[0]?.at.slice(0, 10);
  const lines: TrailLine[] = [];
  for (const one of steps) {
    const said: string[] = [];
    switch (one.step) {
      case "asked":
        said.push(TRAIL.asked(task));
        break;
      case "shown":
        said.push(TRAIL.handed(asker));
        break;
      case "passed_on":
        said.push(one.noted ? TRAIL.passedNote(asker) : TRAIL.passed(asker));
        break;
      case "reached":
        if (one.why === "no_reply") said.push(TRAIL.noReply(asker));
        if (one.why === "asker_gone") said.push(TRAIL.gone(asker));
        said.push(TRAIL.reached);
        break;
      case "answered":
        if (one.by === "asking_chat") said.push(TRAIL.chatAnswered(asker));
        if (one.by === "person") said.push(TRAIL.youAnswered);
        break;
      case "closed":
        // Closed by an answer, the answer has said it.
        if (one.by === "nobody") said.push(TRAIL.taskEnded);
        break;
      case "failed":
        said.push(one.what === "not_read_back" ? TRAIL.notReadBack : TRAIL.redirectFailed);
        break;
    }
    const time = timeOf(one.at, day);
    for (const words of said) lines.push({ at: one.at, time, said: words });
  }
  return lines;
}
