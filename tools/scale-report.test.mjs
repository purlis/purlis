// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The nightly scale run's report: each budget of #5486 against what the run measured, and what
// that does to the one `scale-regression` issue. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { bodyFor, decide, lastPassIn, parseArgv, parseRows, summary, verdicts } from "./scale-report.mjs";

const ms = (budget, os, samples) => ({ budget, os, samples });

/** A run in which every budgeted measurement was taken on both systems. */
const everyBudget = (hookMs) => [
  ms("hook", "macos", [hookMs, 20, 21]),
  ms("hook", "linux", [10, 11, 12]),
  ms("recall", "macos", [40]),
  ms("recall", "linux", [40]),
  ms("recall-all", "macos", [60]),
  ms("recall-all", "linux", [60]),
  ms("watcher-setup", "macos", [400]),
  ms("watcher-setup", "linux", [3000]),
];

const of = (found, budget, os) => found.find((v) => v.budget === budget && v.os === os);

test("a budget is held to the p95 of its samples, and met at the budget itself", () => {
  // Twenty samples: the p95 by nearest rank is the 19th smallest.
  const samples = [...Array.from({ length: 18 }, () => 10), 50, 400];
  const found = verdicts([ms("hook", "linux", samples)]);
  assert.equal(of(found, "hook", "linux").value, 50);
  assert.equal(of(found, "hook", "linux").status, "met");
});

test("a p95 over its budget is a miss, on the system it was measured on only", () => {
  const found = verdicts(everyBudget(1800));
  assert.equal(of(found, "hook", "macos").status, "missed");
  assert.equal(of(found, "hook", "macos").value, 1800);
  assert.equal(of(found, "hook", "linux").status, "met");
});

test("the watcher's one-second budget is macOS's, and Linux's setup is reported against none", () => {
  const found = verdicts(everyBudget(20));
  assert.equal(of(found, "watcher-setup", "macos").status, "met");
  assert.equal(of(found, "watcher-setup", "linux").status, "reported");
});

test("what has no budget is reported, never met or missed", () => {
  const found = verdicts([ms("sidebar", "linux", [9000]), { budget: "idle-cpu", os: "linux", value: 85 }]);
  assert.equal(of(found, "sidebar", "linux").status, "reported");
  assert.equal(of(found, "idle-cpu", "linux").status, "reported");
  assert.equal(of(found, "idle-cpu", "linux").value, 85);
});

test("a budget whose subject is not built yet is pending, on every system", () => {
  const found = verdicts([]);
  for (const os of ["macos", "linux"]) {
    assert.equal(of(found, "project-search", os).status, "pending");
    assert.equal(of(found, "index-rebuild", os).status, "pending");
  }
});

test("a budget that should have been measured and was not is said to be not measured", () => {
  const found = verdicts([ms("hook", "linux", [10])]);
  assert.equal(of(found, "hook", "macos").status, "not measured");
  assert.equal(of(found, "recall", "linux").status, "not measured");
});

test("a miss opens the issue when there is none, and reopens a closed one", () => {
  const missed = verdicts(everyBudget(1800));
  assert.deepEqual(decide(missed, undefined), { action: "open" });
  assert.deepEqual(decide(missed, { number: 7, state: "closed" }), { action: "open", number: 7 });
});

test("a miss with the issue already open updates it", () => {
  assert.deepEqual(decide(verdicts(everyBudget(1800)), { number: 7, state: "open" }), {
    action: "update",
    number: 7,
  });
});

test("a pass closes an open issue, and leaves a closed or absent one alone", () => {
  const passed = verdicts(everyBudget(20));
  assert.deepEqual(decide(passed, { number: 7, state: "open" }), { action: "close", number: 7 });
  assert.deepEqual(decide(passed, { number: 7, state: "closed" }), { action: "none" });
  assert.deepEqual(decide(passed, undefined), { action: "none" });
});

test("a run that measured too little to say is neither a miss nor a pass, and touches no issue", () => {
  const broken = verdicts([ms("hook", "linux", [10])]);
  assert.deepEqual(decide(broken, { number: 7, state: "open" }), { action: "none" });
  assert.deepEqual(decide(broken, undefined), { action: "none" });
});

test("a miss is a miss even where another budget went unmeasured", () => {
  const found = verdicts([ms("hook", "linux", [900])]);
  assert.deepEqual(decide(found, undefined), { action: "open" });
});

test("the last pass is kept in the issue's body and read back from it", () => {
  const body = bodyFor({
    report: "## Scale run\n",
    head: "abc1234def",
    lastPass: "0123456789abcdef",
    commits: [{ sha: "abc1234def5678", subject: "Make the hook slow (#9)" }],
  });
  assert.equal(lastPassIn(body), "0123456789abcdef");
  assert.match(body, /abc1234 Make the hook slow \(#9\)/);
  assert.equal(lastPassIn("an issue written by a person"), undefined);
});

test("with no pass recorded the body says so instead of listing commits", () => {
  const body = bodyFor({ report: "", head: "abc", lastPass: undefined, commits: [] });
  assert.match(body, /no pass recorded yet/);
  assert.equal(lastPassIn(body), undefined);
});

test("the summary names every budget, its value against its budget and its status", () => {
  const text = summary(verdicts(everyBudget(1800)), { head: "abc1234" });
  assert.match(text, /\| SessionStart hook \| macOS \| p95 1800 ms \| ≤ 50 ms \| missed \|/);
  assert.match(text, /\| Project search \| Linux \| — \| < 100 ms \| pending \(#5498\) \|/);
});

test("rows are read one JSON object a line, blank lines skipped", () => {
  assert.deepEqual(parseRows('{"budget":"hook","os":"linux","samples":[1]}\n\n'), [ms("hook", "linux", [1])]);
});

test("the report's arguments are its row files, with --issue and --only <system> anywhere among them", () => {
  assert.deepEqual(parseArgv(["--only", "linux", "a.jsonl", "--issue", "b.jsonl"]), {
    files: ["a.jsonl", "b.jsonl"],
    issue: true,
    only: "linux",
  });
  assert.deepEqual(parseArgv(["a.jsonl"]), { files: ["a.jsonl"], issue: false, only: undefined });
  assert.throws(() => parseArgv(["--only", "windows"]), /--only takes macos or linux/);
});

test("a report of one system holds that system's verdicts alone", () => {
  const found = verdicts(everyBudget(20), { only: "linux" });
  assert.ok(found.length > 0);
  assert.ok(found.every((v) => v.os === "linux"));
});
