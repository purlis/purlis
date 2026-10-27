// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { FinishedTask, RunnerAway } from "./bindings";
import { nothingKnown, type ChatStates } from "./chatState";
import { awaySummaryOf, type Away } from "./awayCounts";
import { awayByRunner, countedOnRunners, runnerSaid } from "./awayRunners";

const at = (iso: string) => Date.parse(iso);
const AWAY: Away[] = [{ from: at("2026-10-27T01:00:00Z"), to: at("2026-10-27T07:00:00Z") }];
const names: Record<number, string> = { 1: "steward 1", 2: "qa 2", 3: "dev 3", 5: "dev 5" };
const nameOf = (session: number) => names[session] ?? String(session);

function finished(id: string, chat: number | null, more: Partial<FinishedTask> = {}): FinishedTask {
  return {
    id,
    asker: 1,
    name: `task ${id}`,
    chat,
    persona: "dev",
    how: "done",
    outcome: "done",
    folds: true,
    report: "",
    changed: null,
    ended: "2026-10-27T03:00:00Z",
    place: "alpha",
    branch: null,
    reopens: true,
    not_reopened: null,
    did_not_start: false,
    attempts: 0,
    waits: null,
    ...more,
  };
}

function runner(pool: string, chats: number[], more: Partial<RunnerAway> = {}): RunnerAway {
  return { pool, chats, branches: [], log: "read", ended: [], ...more };
}

const RUNNER = "01JB0RUNNER0000000000000000";
const split = (summary: ReturnType<typeof counted>, runners: RunnerAway[]) =>
  awayByRunner(summary, runners, { away: AWAY, nameOf });

function counted(tasks: FinishedTask[], waiting: number[] = []) {
  const states: ChatStates = { ...nothingKnown, needsYou: waiting };
  return awaySummaryOf({
    away: AWAY,
    finished: tasks,
    cameToNeed: new Set(waiting),
    states,
    nameOf,
  });
}

describe("while you were away, a section per runner (#2051, spec #2041 story 15)", () => {
  it("moves what a runner's chats did into that runner's section, and leaves this machine's", () => {
    const whole = split(
      counted([finished("A", 3), finished("B", 5), finished("C", null)], [2, 5]),
      [runner("qa", [2, 3]), runner("build", [5])],
    );

    expect(whole.here.done.map((one) => one.key)).toEqual(["task:C"]);
    expect(whole.here.waiting).toEqual([]);
    expect(whole.runners.map((one) => one.pool)).toEqual(["qa", "build"]);
    const [qa, build] = whole.runners;
    expect(qa.finished.map((one) => one.key)).toEqual(["task:A"]);
    expect(qa.waiting.map((one) => one.key)).toEqual(["chat:2"]);
    expect(build.finished.map((one) => one.key)).toEqual(["task:B"]);
    expect(build.waiting.map((one) => one.key)).toEqual(["chat:5"]);
  });

  it("counts a failed task of a runner chat as finished there, said apart from the done", () => {
    const whole = split(counted([finished("A", 3, { how: "failed", outcome: "failed" })]), [
      runner("qa", [3]),
    ]);
    expect(whole.here.failed).toEqual([]);
    expect(whole.runners[0].failed.map((one) => one.key)).toEqual(["task:A"]);
  });

  it("names the branches ready to push only of a runner something happened on", () => {
    const ready = [
      {
        workspace: "alpha",
        repo: "app",
        branches: [
          { pool: "qa", branch: "fix-login", ahead: 2 },
          { pool: "qa", branch: "pushed", ahead: 0 },
        ],
      },
    ];
    const whole = split(counted([finished("A", 3)]), [
      runner("qa", [3], { branches: ready }),
      runner("idle", [9], { branches: ready }),
    ]);

    expect(whole.runners.map((one) => one.pool)).toEqual(["qa"]);
    expect(whole.runners[0].branches).toEqual([
      {
        key: "branch:alpha/app/fix-login",
        says: "fix-login in app, 2 commits ahead",
        go: { to: "changes", workspace: "alpha" },
      },
    ]);
  });

  it("says each runner's section in its own sentence", () => {
    const whole = split(counted([finished("A", 3), finished("B", 3)], [2]), [
      runner("qa", [2, 3], {
        branches: [
          { workspace: "alpha", repo: "app", branches: [{ pool: "qa", branch: "fix", ahead: 1 }] },
        ],
      }),
    ]);
    expect(runnerSaid(whole.runners[0])).toBe(
      "On qa's runner: 2 finished, 1 waiting on you, 1 branch ready to push",
    );
  });

  it("is this machine's summary as it was where no chat runs on a runner", () => {
    const summary = counted([finished("A", 3)], [2]);
    const whole = split(summary, []);
    expect(whole.here).toEqual(summary);
    expect(whole.runners).toEqual([]);
  });

  it("says each run its log says ended there while away, at its time here (#3393)", () => {
    const at = Date.parse("2026-10-27T03:12:00Z") / 1000;
    const whole = split(counted([]), [
      runner("qa", [3], {
        ended: [
          { chat: 3, ended: "completed", at, device: RUNNER },
          {
            chat: 5,
            ended: "failed",
            at: Date.parse("2026-10-26T23:00:00Z") / 1000,
            device: RUNNER,
          },
        ],
      }),
    ]);

    expect(whole.runners).toHaveLength(1);
    const [qa] = whole.runners;
    const time = new Date(at * 1000).toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
    expect(qa.finished).toEqual([
      {
        key: `run:${RUNNER}:3:${at}`,
        says: `dev 3 finished at ${time}`,
        go: { to: "chat", session: 3 },
      },
    ]);
  });

  it("does not say a run's end again for a task it already counts", () => {
    const at = Date.parse("2026-10-27T03:00:00Z") / 1000;
    const whole = split(counted([finished("A", 3)]), [
      runner("qa", [3], { ended: [{ chat: 3, ended: "completed", at, device: RUNNER }] }),
    ]);
    expect(whole.runners[0].finished.map((one) => one.key)).toEqual(["task:A"]);
  });

  it("says a runner whose link is down is not heard from, not that nothing happened", () => {
    const whole = split(counted([]), [runner("qa", [3], { log: "link_down" })]);
    expect(whole.runners).toHaveLength(1);
    expect(runnerSaid(whole.runners[0])).toBe(
      "On qa's runner: its link is down, so what happened there is not known yet",
    );
    expect(countedOnRunners(whole.runners)).toBe(1);
  });

  it("says when some of what a runner recorded was not read", () => {
    const at = Date.parse("2026-10-27T03:00:00Z") / 1000;
    const whole = split(counted([]), [
      runner("qa", [3], {
        log: "partly",
        ended: [{ chat: 3, ended: "completed", at, device: RUNNER }],
      }),
    ]);
    expect(runnerSaid(whole.runners[0])).toBe(
      "On qa's runner: 1 finished, and some of what happened there was not read",
    );
  });

  it("does not count a run that failed or was stopped as finished", () => {
    const at = Date.parse("2026-10-27T03:00:00Z") / 1000;
    const whole = split(counted([]), [
      runner("qa", [3, 5], {
        ended: [
          { chat: 3, ended: "failed", at, device: RUNNER },
          { chat: 5, ended: "stopped", at, device: RUNNER },
        ],
      }),
    ]);
    const [qa] = whole.runners;
    expect(qa.finished).toEqual([]);
    expect(qa.failed.map((one) => one.key)).toEqual([`run:${RUNNER}:3:${at}`]);
    expect(qa.stopped.map((one) => one.key)).toEqual([`run:${RUNNER}:5:${at}`]);
    expect(runnerSaid(qa)).toBe("On qa's runner: 1 failed, 1 stopped");
  });
});
