// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Which `stories` run a pull request's baselines are taken from, and whether its baselines are
// of the head (`tools/story-baselines-pull.mjs`). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { notOf, runToTake } from "./story-baselines-pull.mjs";

const HEAD = "a".repeat(40);
const BASE = "b".repeat(40);
const FROM = "purlis/purlis";

const runOf = (over) => ({
  id: 1,
  run_number: 1,
  run_attempt: 1,
  head_sha: HEAD,
  event: "pull_request",
  head_repository: { full_name: FROM },
  status: "completed",
  conclusion: "failure",
  html_url: "https://github.com/purlis/purlis/actions/runs/1",
  ...over,
});

test("the newest finished, failed run of the head is the one taken", () => {
  const newest = runOf({ id: 3, run_number: 3 });
  assert.equal(
    runToTake([runOf({ id: 2, run_number: 2 }), newest, runOf({ id: 9, run_number: 9, head_sha: BASE })], HEAD, FROM)
      .run,
    newest,
  );
  // A re-run of the same run is newer than its first attempt.
  const again = runOf({ id: 4, run_number: 4, run_attempt: 2 });
  assert.equal(runToTake([runOf({ id: 4, run_number: 4 }), again], HEAD, FROM).run, again);
});

test("only a pull_request run of the head's own repository is taken", () => {
  assert.match(
    runToTake([runOf({ event: "workflow_dispatch" })], HEAD, FROM).why,
    /no stories run has tested/,
  );
  assert.match(
    runToTake([runOf({ head_repository: { full_name: "someone/fork" } })], HEAD, FROM).why,
    /no stories run has tested/,
  );
  const mine = runOf({ id: 2, run_number: 2 });
  assert.equal(
    runToTake([mine, runOf({ id: 3, run_number: 3, event: "workflow_dispatch" })], HEAD, FROM).run,
    mine,
  );
});

test("no run, a run still going, or a run that passed is said, and nothing is taken", () => {
  assert.match(runToTake([], HEAD, FROM).why, /no stories run has tested/);
  assert.match(runToTake([runOf({ head_sha: BASE })], HEAD, FROM).why, /no stories run has tested/);
  assert.match(
    runToTake([runOf({ status: "in_progress", conclusion: null })], HEAD, FROM).why,
    /has not finished/,
  );
  assert.match(runToTake([runOf({ conclusion: "success" })], HEAD, FROM).why, /passed/);
  // An older failed run is not taken past a newer one that passed: the newest says what is so.
  assert.match(
    runToTake([runOf({ run_number: 1 }), runOf({ run_number: 2, conclusion: "success" })], HEAD, FROM)
      .why,
    /passed/,
  );
});

test("baselines drawn of another commit, or that name no base, are refused", () => {
  assert.equal(notOf({ head: HEAD, base: BASE }, HEAD), undefined);
  assert.match(notOf({ head: BASE, base: BASE }, HEAD), /drawn of/);
  assert.match(notOf({ head: HEAD }, HEAD), /which base/);
  assert.match(notOf({ head: HEAD, base: "main" }, HEAD), /which base/);
  assert.match(notOf(null, HEAD), /which commit/);
  assert.match(notOf([], HEAD), /which commit/);
});
