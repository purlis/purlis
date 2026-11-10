// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { InboxUpdate, QuietEnded, Shown } from "./bindings";
import { quietPartsSaid, quietSaid, quietSummaryOf } from "./quiet";
import { askSeenKey, updateSeenKey } from "./seenRows";

const ask = (session: number, key: string): Shown => ({
  session,
  ask: key,
  says: key,
  options: [],
  source: "permission",
  chain: [`chat ${session}`],
  answer: { via: "in-its-pane" },
});

const update = (key: string, kind: InboxUpdate["kind"], at: number, read = false): InboxUpdate => ({
  key,
  kind,
  at,
  session: 3,
  chain: ["chat 3"],
  says: key,
  read,
});

const ended: QuietEnded = {
  from: 1_000,
  to: 2_000,
  asks: [
    { plane: "/a", key: "p1" },
    { plane: "/a", key: "p2" },
    { plane: "/a", key: "p3" },
    { plane: "/b", key: "p9" },
  ],
};

const everyUpdate = [
  update("done-before", "task-done", 999),
  update("done-1", "task-done", 1_000),
  update("done-2", "task-done", 2_000),
  update("done-after", "task-done", 2_001),
  update("failed-1", "task-failed", 1_500),
  update("doctor", "doctor", 1_500),
  update("done-read", "task-done", 1_500, true),
];

const nothingSeen = () => true;

describe("the summary of a Quiet that ended", () => {
  it("counts the asks that came in it, still waiting, and the tasks that finished or failed in it", () => {
    const asks = [ask(3, "p1"), ask(3, "p2"), ask(4, "p3"), ask(5, "before-quiet")];
    const summary = quietSummaryOf(ended, "/a", asks, everyUpdate, nothingSeen);
    expect(summary.asks.map((one) => one.ask)).toEqual(["p1", "p2", "p3"]);
    expect(summary.finished.map((one) => one.key)).toEqual(["done-1", "done-2"]);
    expect(summary.failed.map((one) => one.key)).toEqual(["failed-1"]);
    expect(quietSaid(summary)).toBe("While you were quiet: 3 asks, 2 finished, 1 failed");
  });

  it("counts only this project's asks, and none answered since", () => {
    const summary = quietSummaryOf(ended, "/a", [ask(3, "p2"), ask(9, "p9")], [], nothingSeen);
    expect(summary.asks.map((one) => one.ask)).toEqual(["p2"]);
    expect(quietPartsSaid(summary)).toEqual([{ part: "asks", says: "1 ask" }]);
  });

  it("drops each part once it is seen, and is nothing once all of it is", () => {
    const asks = [ask(3, "p1")];
    const updates = [
      update("done-1", "task-done", 1_000),
      update("failed-1", "task-failed", 1_500),
    ];
    const seen = new Set([askSeenKey(asks[0]), updateSeenKey("done-1")]);
    const isNew = (key: string) => !seen.has(key);
    const some = quietSummaryOf(ended, "/a", asks, updates, isNew);
    expect(quietSaid(some)).toBe("While you were quiet: 1 failed");
    seen.add(updateSeenKey("failed-1"));
    const none = quietSummaryOf(ended, "/a", asks, updates, isNew);
    expect(quietPartsSaid(none)).toEqual([]);
    expect(quietSaid(none)).toBe("");
  });

  it("is nothing before any Quiet ended", () => {
    expect(
      quietPartsSaid(quietSummaryOf(null, "/a", [ask(3, "p1")], everyUpdate, nothingSeen)),
    ).toEqual([]);
  });
});
