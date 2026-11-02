// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { Shown, WithAskingChat } from "./bindings";
import {
  askingBelow,
  questionWord,
  standingOf,
  waitingOnYou,
  type QuestionsHeld,
} from "./taskQuestions";

/**
 * **One state per task question** (#4564, spec #4551): derived from the asks registry alone,
 * so the row, the tab, the ✋ and the Inbox cannot disagree about it.
 */

const reached = (session: number, since: number): Shown => ({
  session,
  ask: `task-question:${session}:1`,
  says: "Which host?",
  options: [],
  source: "task-question",
  chain: ["steward 2", `developer ${session}`],
  answer: { via: "task-question", why: "no-reply", from: "steward 2", note: null },
  since,
  held_until: null,
  writes: null,
  lines: null,
  last_message: null,
});

const permission = (session: number): Shown => ({
  ...reached(session, 0),
  ask: `perm-${session}`,
  source: "permission",
  answer: { via: "in-its-pane" },
});

const withAsker = (session: number, since: number): WithAskingChat => ({
  session,
  asker: "steward 2",
  since,
});

const held = (asks: Shown[], with_asking_chat: WithAskingChat[] = []): QuestionsHeld => ({
  asks,
  with_asking_chat,
});

describe("a task question's state", () => {
  it("is with its asking chat, named, from when it was asked", () => {
    expect(standingOf(held([], [withAsker(7, 1_000)]), 7)).toEqual({
      level: "with-asker",
      asker: "steward 2",
      since: 1_000,
    });
  });

  it("waits on you from when it reached you, once the registry lists it", () => {
    expect(standingOf(held([reached(7, 2_000)]), 7)).toEqual({ level: "needs-you", since: 2_000 });
  });

  it("is nothing for a chat with no task question, whatever else it asks", () => {
    expect(standingOf(held([permission(7)]), 7)).toBeUndefined();
    expect(standingOf(undefined, 7)).toBeUndefined();
  });
});

describe("what waits on you", () => {
  it("is each task question that reached you and never one with its asking chat", () => {
    expect(waitingOnYou(held([reached(7, 1)], [withAsker(8, 1)]))).toEqual([7]);
  });
});

describe("a row's words", () => {
  it("say who it waits on and for how long: now, then whole minutes", () => {
    const now = new Date(100_000 * 1000);
    expect(
      questionWord({ level: "with-asker", asker: "steward 2", since: 100_000 - 40 }, now),
    ).toBe("asking steward 2 · now");
    expect(questionWord({ level: "needs-you", since: 100_000 - 185 }, now)).toBe("needs you · 3m");
  });
});

describe("a folded row", () => {
  const rows = [
    { session: 1, level: 1 },
    { session: 2, level: 2 },
    { session: 3, level: 3 },
    { session: 4, level: 2 },
    { session: 5, level: 1 },
  ];

  it("counts the questions with an asking chat below it, at any depth", () => {
    const counts = askingBelow(rows, held([reached(4, 1)], [withAsker(3, 1), withAsker(2, 1)]));
    expect(counts.get(1)).toBe(2);
    expect(counts.get(2)).toBe(1);
    expect(counts.has(5)).toBe(false);
  });
});
