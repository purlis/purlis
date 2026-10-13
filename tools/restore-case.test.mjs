// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The `restore` case's reading and verdict (ADR 0086 row L8). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { restoreIsWhole, restoreMarks, restoreTable, restoreVerdict } from "./restore-case.mjs";

test("the marks are read from what the app printed, the first of each", () => {
  const said =
    "charter-bench first-frame 410\nsomething else\ncharter-bench usable 520\n" +
    "charter-bench first-chat 700\ncharter-bench first-frame 9999\ncharter-bench other 1\n";
  const marks = restoreMarks(said);
  assert.deepEqual(marks, { "first-frame": 410, usable: 520, "first-chat": 700 });
  assert.equal(restoreIsWhole(marks), false);
  assert.equal(restoreIsWhole(restoreMarks(`${said}charter-bench all-chats 2100\n`)), true);
});

test("a window that is usable as soon with forty chats as with one does not wait on them", () => {
  const launch = (usable, all) => ({ "first-frame": 400, usable, "first-chat": 600, "all-chats": all });
  const said = restoreVerdict({
    40: [launch(560, 9000), launch(540, 9100), launch(600, 8900)],
    1: [launch(500, 700), launch(520, 690), launch(480, 710)],
    10: [launch(530, 2500)],
  });
  assert.deepEqual(
    said.rows.map((row) => [row.chats, row.usable, row["all-chats"]]),
    [
      [1, 500, 700],
      [10, 530, 2500],
      [40, 560, 9000],
    ],
  );
  assert.equal(said.usableGrowsByMs, 60);
  assert.equal(said.verdict, "the window does not wait on the chats");
  assert.match(restoreTable(said), /\| 40 \| 400 ms \| 560 ms \| 600 ms \| 9000 ms \|/);
});

test("a window usable seconds later with forty chats waits on them, and the case says so", () => {
  const launch = (usable) => ({ "first-frame": 400, usable, "first-chat": usable, "all-chats": usable });
  const said = restoreVerdict({ 1: [launch(600)], 40: [launch(9500)] });
  assert.equal(said.verdict, "the window waits on the chats");
  assert.match(restoreTable(said), /usable 8900 ms later/);
});

test("a build that says no such mark is not measured, and nothing is claimed of it", () => {
  const said = restoreVerdict({ 1: [{ "first-frame": 400 }], 40: [{ "first-frame": 420 }] });
  assert.equal(said.verdict, "not measured");
  assert.equal(said.usableGrowsByMs, undefined);
  assert.match(restoreTable(said), /Verdict: not measured\./);
});
