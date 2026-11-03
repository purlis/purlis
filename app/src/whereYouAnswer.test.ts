// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import type { Shown } from "./bindings";
import { forgetThisLaunch } from "./regions";
import { GLOBAL, sayAboutThisMachine } from "./windowprefs";
import {
  loadWhereYouAnswer,
  quizIn,
  setWhereYouAnswer,
  whereYouAnswer,
  WHERE_YOU_ANSWER,
} from "./whereYouAnswer";

beforeEach(() => {
  forgetThisLaunch();
  sayAboutThisMachine("questions", undefined);
});
afterEach(() => {
  Reflect.deleteProperty(globalThis, GLOBAL);
  clearMocks();
});

describe("where you answer, as the layout file keeps it (#4565)", () => {
  it("is from the top, and not yet offered, until the person says otherwise", () => {
    expect(loadWhereYouAnswer({ version: 1, regions: [] })).toEqual({
      prefs: { answer: "top", offered: false },
      said: [],
    });
  });

  it("reads from each session, and that the choice was offered", () => {
    expect(loadWhereYouAnswer({ questions: { answer: "each", offered: true } }).prefs).toEqual({
      answer: "each",
      offered: true,
    });
  });

  it("says what it put right, and falls back field by field", () => {
    const { prefs, said } = loadWhereYouAnswer({ questions: { answer: "left", offered: 3 } });
    expect(prefs).toEqual({ answer: "top", offered: false });
    expect(said).toEqual([
      `"questions.answer" "left" is not "top" or "each", so it is "top"`,
      `"questions.offered" 3 is not true or false, so it is false`,
    ]);
  });

  it("writes nothing at the default, and only what moved from it", () => {
    expect(WHERE_YOU_ANSWER.written()).toBeUndefined();
    setWhereYouAnswer({ offered: true });
    expect(whereYouAnswer()).toEqual({ answer: "top", offered: true });
    expect(WHERE_YOU_ANSWER.written()).toEqual({ offered: true });
    setWhereYouAnswer({ answer: "each" });
    expect(WHERE_YOU_ANSWER.written()).toEqual({ answer: "each", offered: true });
  });
});

/** A task question that has reached the person, as the asks registry lists it. */
function reached(task: number, since: number, chain: string[] = ["steward 4"]): Shown {
  return {
    session: task,
    ask: `task-question:${task}:1`,
    says: `Question of ${task}?`,
    options: [],
    source: "task-question",
    chain,
    answer: { via: "task-question", why: "passed-on", from: "steward 4", note: null },
    since,
  };
}

const permission: Shown = {
  session: 12,
  ask: "perm-1",
  says: "Run rm -rf build",
  options: [],
  source: "permission",
  chain: ["steward 4"],
  answer: { via: "in-its-pane" },
  since: 5,
} as unknown as Shown;

/** Tasks 11, 12 and 13 are at home in steward 4's tab (chat 4); 21 in talk 2's (chat 20). */
const topOf = (task: number) => (task >= 20 ? 20 : task > 4 ? 4 : undefined);

describe("which question a pane draws as a quiz (#4565)", () => {
  const asks = [reached(12, 200), reached(11, 100), reached(21, 50), permission];

  it("draws the top's oldest question, from the top, with a count of the rest", () => {
    const quiz = quizIn(asks, 4, "top", topOf);
    expect(quiz?.ask.session).toBe(11);
    expect(quiz?.more).toBe(1);
    expect(quiz?.own).toBe(false);
  });

  it("draws nothing at the top from each session", () => {
    expect(quizIn(asks, 4, "each", topOf)).toBeUndefined();
  });

  it("draws a task's own question in its own pane, either way", () => {
    for (const where of ["top", "each"] as const) {
      const quiz = quizIn(asks, 12, where, topOf);
      expect(quiz?.ask.session).toBe(12);
      expect(quiz?.own).toBe(true);
      expect(quiz?.more).toBe(0);
    }
  });

  it("never draws another ask as a quiz: it stays on the task and in the Inbox", () => {
    expect(quizIn([permission], 4, "top", topOf)).toBeUndefined();
    expect(quizIn([permission], 12, "top", topOf)).toBeUndefined();
  });

  it("draws nothing for a question whose top has no tab", () => {
    expect(quizIn([reached(3, 1)], 4, "top", topOf)).toBeUndefined();
  });

  it("draws the next question as soon as one is answered, whatever the order listed", () => {
    expect(quizIn([reached(12, 200)], 4, "top", topOf)?.ask.session).toBe(12);
  });
});
