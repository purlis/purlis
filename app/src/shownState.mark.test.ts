// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { MARK_STATES, shownState, type Facts, type MarkState, type TaskFacts } from "./shownState";

/**
 * **A chat's one state** (#5340, spec #5339): when several are true the first of the fixed order
 * wins. Each row names what is true of the chat and the state its mark then wears.
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

const table: readonly [string, Partial<Facts>, MarkState | null][] = [
  // Each state alone.
  ["its turn ended on the person", { needsYou: true }, "needs"],
  ["a task of its failed and it has not been seen", { ...FAILED_BELOW }, "failed"],
  ["its turn runs", { board: "running" }, "working"],
  ["its turn ended while tasks work", { ...AT_WORK_BELOW }, "waiting"],
  [
    "its turn ended on work of its own",
    { ownWork: { helpers: 1, workflows: 0, monitors: 0, shells: 0 } },
    "waiting",
  ],
  ["it is asking the chat that asked it", { task: report({ report: "owed", asking: "a" }) }, "waiting"],
  ["its turn ended and nothing is owed", {}, "idle"],
  ["its program ended", { board: "done" }, "ended"],
  ["its program ended badly and that has been seen", { board: "failed" }, "ended"],
  ["it is a task that reported", { board: undefined, task: report({}) }, "ended"],
  ["it is a task that was seen to fail", { task: report({ outcome: "failed" }) }, "ended"],

  // Every pair, the first of the order winning.
  ["needs you over a failed task", { needsYou: true, ...FAILED_BELOW }, "needs"],
  ["needs you over working", { needsYou: true, board: "running" }, "needs"],
  ["needs you over waiting on tasks", { needsYou: true, ...AT_WORK_BELOW }, "needs"],
  ["needs you over ended", { needsYou: true, board: "done" }, "needs"],
  ["a failed task over working", { board: "running", ...FAILED_BELOW }, "failed"],
  ["a failed task over waiting on tasks", { ...AT_WORK_BELOW, ...FAILED_BELOW }, "failed"],
  ["a failed task over idle", { ...FAILED_BELOW }, "failed"],
  ["a failed task over ended", { board: "done", ...FAILED_BELOW }, "failed"],
  ["working over waiting on tasks", { board: "running", ...AT_WORK_BELOW }, "working"],

  // A failed task on its own chat: its mark says so until it has been seen.
  [
    "it is a task that failed and has not been seen",
    { task: report({ outcome: "failed" }), failedUnseen: true },
    "failed",
  ],
  ["needs you over its own unseen failure", { needsYou: true, failedUnseen: true }, "needs"],

  // Nothing known is nothing guessed: the word says so, and no state is claimed.
  ["a harness that has said nothing", { board: "unknown" }, null],
];

describe("a chat's one state, first true wins (#5340)", () => {
  it("is in the order the spec fixes", () => {
    expect(MARK_STATES).toEqual(["needs", "failed", "working", "waiting", "idle", "ended"]);
  });

  it.each(table)("%s", (_, facts, state) => {
    expect(shownState(chat(facts))?.mark).toBe(state);
  });

  it("has no state for a shell nothing has reported for", () => {
    expect(shownState(chat({ board: undefined }))).toBeUndefined();
  });

  it("says how many tasks failed, in the word its mark stands for", () => {
    expect(shownState(chat({ failuresUnseen: 1 }))?.word).toBe("a task failed");
    expect(shownState(chat({ failuresUnseen: 3 }))?.word).toBe("3 tasks failed");
  });

  it("keeps a failed task's own word on its own mark", () => {
    const task = report({ outcome: "failed" });
    expect(shownState(chat({ task, failedUnseen: true }))?.word).toBe("failed");
  });

  it("goes back to the chat's own word once the failure has been seen", () => {
    expect(shownState(chat({ failuresUnseen: 1 }))?.kind).toBe("task-failed");
    expect(shownState(chat({ failuresUnseen: 0 }))?.kind).toBe("idle");
  });
});
