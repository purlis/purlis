// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What the scenario run's project switches took, read from `app/logs/project-switch.jsonl`. Run
// with `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { annotation, readSwitches } from "./switch-notice.mjs";

/** Two rounds among three projects: the first of each round is the launch's project. */
const record = {
  at: 1,
  budgetMs: 200,
  launchChats: 48,
  projects: 3,
  samples: 6,
  p50: 120,
  p95: 900,
  worst: 900,
  samples_ms: [900.4, 110, 120, 700, 130, 100],
  shown_ms: [600, 60, 70, 500, 80, 50],
};

test("says a switch to the project of many chats apart from one to a project of one", () => {
  const line = annotation(record, "macos-15, part 2 of 4");
  assert.equal(
    line,
    "::notice title=Project switches (macos-15%2C part 2 of 4)::" +
      "To the launch's project (48 chats), press to its pane painted: 900, 700 ms%0A" +
      "To a project of one chat, press to its pane painted: median 120 ms, worst 130 ms%0A" +
      "Press to the project in front, before its pane: launch's 600, 500 ms; one chat median 70 ms, worst 80 ms%0A" +
      "All 6 switches among 3 projects: p50 120 ms, p95 900 ms, worst 900 ms",
  );
});

test("reads the last whole record, past a line cut short", () => {
  const text = `${JSON.stringify({ ...record, p50: 1 })}\n${JSON.stringify(record)}\n{ cut short`;
  assert.equal(readSwitches(text).p50, 120);
});

test("says nothing for a part of the job that ran no switch spec", () => {
  assert.equal(annotation(readSwitches(""), "linux"), undefined);
  assert.equal(annotation({ at: 1 }, "linux"), undefined);
});

test("leaves the line about the project in front out of a record that has no such samples", () => {
  const { shown_ms: _, ...older } = record;
  assert.doesNotMatch(annotation(older, "linux"), /before its pane/);
});
