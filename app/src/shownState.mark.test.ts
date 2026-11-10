// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  asksThePerson,
  failedUnseen,
  failuresSeenBy,
  failuresUnseenOf,
  nothingKnown,
} from "./chatState";
import {
  CHAT_STATES,
  CHAT_STATE_SAID,
  shownState,
  type ChatState,
  type Facts,
  type TaskFacts,
} from "./shownState";

/**
 * **A chat's one state** (#5340, #6012, ADR 0121 decision 17): one of the eight Chat states, and
 * when several are true the first of the fixed order wins. Each row names what is true of the
 * chat and the state its mark then wears.
 */
const chat = (more: Partial<Facts>): Facts => ({
  board: "waiting",
  needsYou: false,
  task: null,
  harness: "Claude Code",
  ...more,
});
const report = (how: Partial<TaskFacts>): TaskFacts => ({
  report: "sent",
  outcome: "done",
  asking: null,
  ...how,
});
const FAILED_BELOW = { failuresUnseen: 1 };
const AT_WORK_BELOW = { tasksAtWork: 2 };
const OWN_WORK = { ownWork: { helpers: 1, workflows: 0, monitors: 0, shells: 0 } };

const table: readonly [string, Partial<Facts>, ChatState][] = [
  // Each state alone.
  ["its turn ended on the person", { needsYou: true }, "needs"],
  ["its turn runs with nothing new for too long", { board: "running", stuck: true }, "stuck"],
  ["a task of its failed and it has not been seen", { ...FAILED_BELOW }, "failed"],
  ["its turn runs", { board: "running" }, "working"],
  // ADR 0076 (#3878): a chat waiting on its own background work is not waiting on anyone.
  ["its turn ended on work of its own", { ...OWN_WORK }, "working"],
  ["its turn ended while tasks work", { ...AT_WORK_BELOW }, "waiting"],
  [
    "it is asking the chat that asked it",
    { task: report({ report: "owed", asking: "a" }) },
    "waiting",
  ],
  // A dispatched task reports to the chat that dispatched it (ADR 0107): its turn end waits on
  // that chat, not the person (#6036).
  [
    "it is a task whose turn ended owing its report",
    { task: report({ report: "owed" }) },
    "waiting",
  ],
  ["its turn ended and nothing is owed", {}, "idle"],
  // Idle is finished with work left behind (#6014): a clean folder is Done.
  ["its turn ended with work left in its folder", { leftWork: true }, "idle"],
  ["its turn ended with nothing left in its folder", { leftWork: false }, "ended"],
  ["its program ended", { board: "done" }, "ended"],
  ["it is a task that reported", { board: undefined, task: report({}) }, "ended"],
  ["it is a task the person stopped", { task: report({ outcome: "stopped_by_person" }) }, "ended"],
  ["its program ended badly", { board: "failed" }, "failed"],
  ["it is a task that was seen to fail", { task: report({ outcome: "failed" }) }, "failed"],
  [
    "it is a task that ended without a report",
    { board: "done", task: report({ report: "owed" }) },
    "failed",
  ],
  ["a harness that has said nothing", { board: "unknown" }, "cant-tell"],
  ["a shell nothing has reported for", { board: undefined }, "cant-tell"],

  // Every pair, the first of the order winning.
  ["waiting on you over stuck", { needsYou: true, board: "running", stuck: true }, "needs"],
  ["waiting on you over a failed task", { needsYou: true, ...FAILED_BELOW }, "needs"],
  ["waiting on you over working", { needsYou: true, board: "running" }, "needs"],
  ["waiting on you over waiting on tasks", { needsYou: true, ...AT_WORK_BELOW }, "needs"],
  ["waiting on you over ended", { needsYou: true, board: "done" }, "needs"],
  ["stuck over a failed task", { board: "running", stuck: true, ...FAILED_BELOW }, "stuck"],
  ["a failed task over working", { board: "running", ...FAILED_BELOW }, "failed"],
  ["a failed task over waiting on tasks", { ...AT_WORK_BELOW, ...FAILED_BELOW }, "failed"],
  ["a failed task over idle", { ...FAILED_BELOW }, "failed"],
  ["a failed task over ended", { board: "done", ...FAILED_BELOW }, "failed"],
  ["working over waiting on tasks", { board: "running", ...AT_WORK_BELOW }, "working"],
  ["waiting on tasks over its own work", { ...AT_WORK_BELOW, ...OWN_WORK }, "waiting"],
  // Stuck? is a question about a chat at work: a chat that is not working is not stuck.
  ["stuck only while it works", { stuck: true }, "idle"],

  // A failed task on its own chat: its mark says so until it has been seen.
  [
    "it is a task that failed and has not been seen",
    { task: report({ outcome: "failed" }), failedUnseen: true },
    "failed",
  ],
  ["waiting on you over its own unseen failure", { needsYou: true, failedUnseen: true }, "needs"],
];

describe("a chat's one state, first true wins (#5340, #6012)", () => {
  it("is one of the eight, in the order that wins", () => {
    expect(CHAT_STATES).toEqual([
      "needs",
      "stuck",
      "failed",
      "working",
      "waiting",
      "idle",
      "ended",
      "cant-tell",
    ]);
  });

  it.each(table)("%s", (_, facts, state) => {
    expect(shownState(chat(facts)).mark).toBe(state);
  });

  it("names every state it can reach", () => {
    expect(new Set(table.map(([, , state]) => state))).toEqual(new Set(CHAT_STATES));
  });

  it("says what is not known in words, naming the harness", () => {
    expect(shownState(chat({ board: "unknown" })).word).toBe(
      "running (no detail from Claude Code)",
    );
    expect(shownState(chat({ board: undefined })).word).toBe("a shell (no detail)");
  });

  it("asks whether a stuck chat is stuck, in words", () => {
    expect(shownState(chat({ board: "running", stuck: true })).word).toBe("stuck?");
  });

  it("says a finished chat with nothing left in its folder is done (#6014)", () => {
    expect(shownState(chat({ leftWork: false })).word).toBe("done");
    expect(shownState(chat({ leftWork: true })).word).toBe("idle");
  });

  it("says how many tasks failed, in the word its mark stands for", () => {
    expect(shownState(chat({ failuresUnseen: 1 })).word).toBe("a task failed");
    expect(shownState(chat({ failuresUnseen: 3 })).word).toBe("3 tasks failed");
  });

  it("keeps a failed task's own word on its own mark", () => {
    const task = report({ outcome: "failed" });
    expect(shownState(chat({ task, failedUnseen: true })).word).toBe("failed");
  });

  it("goes back to the chat's own word once the failure has been seen", () => {
    expect(shownState(chat({ failuresUnseen: 1 })).kind).toBe("task-failed");
    expect(shownState(chat({ failuresUnseen: 0 })).kind).toBe("idle");
  });

  it("wears one shape and one colour per state, none shared", () => {
    const shapes = CHAT_STATES.map((state) => CHAT_STATE_SAID[state].shape);
    const tokens = CHAT_STATES.map((state) => CHAT_STATE_SAID[state].token);
    expect(new Set(shapes).size).toBe(CHAT_STATES.length);
    expect(new Set(tokens).size).toBe(CHAT_STATES.length);
  });
});

describe("what looking at a chat sees (#5340)", () => {
  const failedTasks = {
    1: [
      { id: "r-own-task", task: "build", chat: 2 },
      { id: "r-gone", task: "lint", chat: null },
    ],
    5: [{ id: "r-other", task: "deploy", chat: 6 }],
  };

  it("sees every failure of the chat's own tasks", () => {
    expect(failuresSeenBy({ failedTasks }, 1)).toEqual([
      { asker: 1, id: "r-own-task" },
      { asker: 1, id: "r-gone" },
    ]);
  });

  it("sees a task's own failure, on the record of the chat that asked for it", () => {
    expect(failuresSeenBy({ failedTasks }, 2)).toEqual([{ asker: 1, id: "r-own-task" }]);
  });

  it("sees nothing of another chat's", () => {
    expect(failuresSeenBy({ failedTasks }, 3)).toEqual([]);
  });

  it("knows a chat as a failed task by its asker's record alone", () => {
    const states = { ...nothingKnown, failedTasks };
    expect(failedUnseen(states, 2)).toBe(true);
    expect(failedUnseen(states, 1)).toBe(false);
    expect(failuresUnseenOf(states, 1)).toBe(2);
    expect(failuresUnseenOf(states, 2)).toBe(0);
  });

  it("asks for the person only when in the queue for more than what failed", () => {
    const states = { ...nothingKnown, needsYou: [1, 2], onlyFailures: { 2: true as const } };
    expect(asksThePerson(states, 1)).toBe(true);
    expect(asksThePerson(states, 2)).toBe(false);
    expect(asksThePerson(states, 3)).toBe(false);
  });
});

describe("a task's record says its program has ended, so nothing live is true of it (#5340)", () => {
  it("is working only once the person has run a reported task again", () => {
    expect(shownState(chat({ board: "running", task: report({}) })).mark).toBe("working");
  });

  it("has failed with no report of its own, whatever the board last heard", () => {
    expect(shownState(chat({ board: "running", task: report({ report: "failed" }) })).mark).toBe(
      "failed",
    );
  });

  it("is done at its report, though tasks below it still work", () => {
    expect(shownState(chat({ task: report({}), ...AT_WORK_BELOW })).mark).toBe("ended");
  });
});
