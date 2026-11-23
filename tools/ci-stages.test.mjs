// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// "A PR run is a subset of the queue run" (#7335, ADR 0127), held against the workflows: a job
// that can run on a pull request and cannot on the merge queue is a check the queue never makes,
// so the pull request's green says more than the queue's. The last tests read this repository's
// own workflows, in the tools' tests of `web`'s part 2, so a new job that breaks the rule fails
// the pull request that adds it.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { MERGE_GROUP, PULL_REQUEST, evaluate } from "./ci-push-jobs.mjs";
import { MIN_REASON, PR_ONLY, canRunOn, strandedOnQueue, triggersOf, workflowProblems } from "./ci-stages.mjs";

const WORKFLOWS = fileURLToPath(new URL("../.github/workflows", import.meta.url));

/** A workflow of `on`'s events and the jobs `body` writes, indented as `jobs:` holds them. */
const workflow = (events, body) =>
  `name: w\non:\n${events.map((e) => `  ${e}:\n`).join("")}jobs:\n${body}`;
const job = (id, ifLine, needs = "") =>
  `  ${id}:\n${needs ? `    needs: [${needs}]\n` : ""}${ifLine ? `    if: ${ifLine}\n` : ""}    runs-on: ubuntu-24.04\n    steps:\n      - run: true\n`;

test("an expression is read as a pull request's run and as the queue's, what it cannot know unknown", () => {
  assert.equal(evaluate("github.event_name == 'pull_request'", PULL_REQUEST), true);
  assert.equal(evaluate("github.event_name == 'pull_request'", MERGE_GROUP), false);
  assert.equal(
    evaluate("${{ !cancelled() && (github.event_name == 'pull_request' || github.event_name == 'merge_group') }}", MERGE_GROUP),
    true,
  );
  assert.equal(evaluate("needs.changes.outputs.a != 'false' && !github.event.pull_request.draft", MERGE_GROUP), undefined);
});

test("the events a workflow triggers on are read from its on: block", () => {
  assert.deepEqual(
    [...triggersOf("name: x\non:\n  pull_request:\n    types: [opened]\n  merge_group:\n    types: [checks_requested]\n  # push:\njobs:\n  a:\n")].sort(),
    ["merge_group", "pull_request"],
  );
  assert.deepEqual([...triggersOf("name: x\non: [push, pull_request]\njobs:\n")].sort(), ["pull_request", "push"]);
});

test("a job that runs on a pull request and not in the queue is stranded", () => {
  const text = workflow(["pull_request", "merge_group"], job("only-pr", "${{ github.event_name == 'pull_request' }}"));
  assert.deepEqual(strandedOnQueue(text), ["only-pr"]);
});

test("a job that runs on both, or on neither, or on the queue alone, is not stranded", () => {
  const text = workflow(
    ["pull_request", "merge_group"],
    [
      job("plain", ""),
      job("both", "${{ !cancelled() && (github.event_name == 'pull_request' || github.event_name == 'merge_group') }}"),
      job("queue-only", "${{ github.event_name == 'merge_group' }}"),
      job("neither", "${{ github.event_name == 'schedule' }}"),
      job("scoped", "${{ needs.changes.outputs.rust != 'false' && !github.event.pull_request.draft }}"),
    ].join(""),
  );
  assert.deepEqual(strandedOnQueue(text), []);
});

test("a job that needs a job the queue skips is skipped there too, unless it runs whatever its needs did", () => {
  const text = workflow(
    ["pull_request", "merge_group"],
    [
      job("a", "${{ github.event_name == 'pull_request' }}"),
      job("follows", "", "a"),
      job("always-runs", "${{ !cancelled() }}", "a"),
    ].join(""),
  );
  assert.deepEqual(strandedOnQueue(text), ["a", "follows"]);
});

test("a job's run on the queue is read with its needs", () => {
  const text = workflow(["merge_group"], job("a", "") + job("b", "", "a"));
  assert.equal(canRunOn(text, "b", MERGE_GROUP), true);
});

test("a workflow that triggers on a pull request and not the queue is named unless it is listed with its reason", () => {
  const text = workflow(["pull_request"], job("a", ""));
  assert.match(workflowProblems({ "x.yml": text }, {}).join("\n"), /x\.yml.*merge_group/);
  assert.deepEqual(
    workflowProblems({ "x.yml": text }, { "x.yml": "an informational probe that gates nothing, so a queue run would cost a runner for no check" }),
    [],
  );
});

test("a listed workflow that runs in the queue now, or is gone, is refused: the listing does nothing", () => {
  const both = workflow(["pull_request", "merge_group"], job("a", ""));
  assert.match(workflowProblems({ "x.yml": both }, { "x.yml": "r" }).join("\n"), /x\.yml.*take it off/);
  assert.match(workflowProblems({}, { "x.yml": "r" }).join("\n"), /x\.yml.*take it off/);
  assert.match(workflowProblems({ "x.yml": workflow(["pull_request"], job("a", "")) }, { "x.yml": "" }).join("\n"), /x\.yml.*reason/);
});

test("a stranded job is a problem of its workflow", () => {
  const text = workflow(["pull_request", "merge_group"], job("only-pr", "${{ github.event_name == 'pull_request' }}"));
  assert.match(workflowProblems({ "w.yml": text }, {}).join("\n"), /only-pr.*pull request.*queue/);
});

const workflows = () =>
  Object.fromEntries(
    readdirSync(WORKFLOWS)
      .filter((name) => name.endsWith(".yml"))
      .map((name) => [name, readFileSync(join(WORKFLOWS, name), "utf8")]),
  );

test("every workflow of this repository holds the rule: a pull request's run is a subset of the queue's", () => {
  assert.deepEqual(workflowProblems(workflows(), PR_ONLY), []);
});

test("the rule fails on this repository's ci.yml with one job made pull-request-only", () => {
  const all = workflows();
  const broken = all["ci.yml"].replace(
    "    if: ${{ !cancelled() && needs.changes.outputs.rust != 'false' }}\n",
    "    if: ${{ !cancelled() && github.event_name == 'pull_request' && needs.changes.outputs.rust != 'false' }}\n",
  );
  assert.notEqual(broken, all["ci.yml"], "the fixture changed ci.yml");
  assert.match(workflowProblems({ ...all, "ci.yml": broken }, PR_ONLY).join("\n"), /rust/);
});

test("the stories workflow runs in the queue, and ci.yml and stories.yml start a ready_for_review run", () => {
  const all = workflows();
  for (const name of ["ci.yml", "stories.yml"]) {
    assert.ok(triggersOf(all[name]).has("merge_group"), `${name} runs in the queue`);
    assert.match(all[name], /pull_request:\n {4}types: \[[^\]]*\bready_for_review\b[^\]]*\]/, name);
  }
});

test("a PR_ONLY reason is a sentence that says why, not a label: a short one is refused", () => {
  const text = workflow(["pull_request"], job("a", ""));
  for (const reason of ["", "   ", "path-filtered", "runs on pull requests"]) {
    assert.match(workflowProblems({ "x.yml": text }, { "x.yml": reason }).join("\n"), /x\.yml.*reason/, JSON.stringify(reason));
  }
});

test("every PR_ONLY entry of this repository carries its own reason, none of them a copy of another's", () => {
  const reasons = Object.values(PR_ONLY);
  for (const [name, reason] of Object.entries(PR_ONLY)) {
    assert.equal(typeof reason, "string", name);
    assert.ok(reason.trim().length >= MIN_REASON, `${name}: a reason is at least ${MIN_REASON} characters`);
  }
  assert.equal(new Set(reasons).size, reasons.length, "no two entries share a reason");
});

const GATED = "${{ !cancelled() && (github.event_name != 'merge_group' || needs.scope.outputs.run != 'false') }}";
const filtered = (extra = "") =>
  `name: w\non:\n  pull_request:\n    paths:\n      - a/**\n  merge_group:\n    types: [checks_requested]\njobs:\n${job("a", GATED, "scope")}${extra}`;
const scopeJob = (file) =>
  `  scope:\n    runs-on: ubuntu-24.04\n    steps:\n      - uses: ./.github/actions/workflow-scope\n        with:\n          workflow: ${file}\n`;

test("a workflow a pull request starts by paths and the queue starts always must scope its queue run", () => {
  assert.match(workflowProblems({ "x.yml": filtered() }, {}).join("\n"), /x\.yml.*paths.*workflow-scope/);
  assert.deepEqual(workflowProblems({ "x.yml": filtered(scopeJob("x.yml")) }, {}), []);
  assert.match(workflowProblems({ "x.yml": filtered(scopeJob("y.yml")) }, {}).join("\n"), /x\.yml.*workflow-scope/);
});

test("a workflow with no path filter needs no scope job in the queue", () => {
  assert.deepEqual(workflowProblems({ "x.yml": workflow(["pull_request", "merge_group"], job("a", "")) }, {}), []);
});

test("a scoped workflow's jobs must need the scope job and its answer; its filter must be a block list", () => {
  const ungated = filtered(scopeJob("x.yml")).replace(`    needs: [scope]\n    if: ${GATED}\n`, "");
  assert.match(workflowProblems({ "x.yml": ungated }, {}).join("\n"), /job "a" must need `scope`/);
  const inline = filtered(scopeJob("x.yml")).replace("    paths:\n      - a/**\n", "    paths: [a/**]\n");
  assert.match(workflowProblems({ "x.yml": inline }, {}).join("\n"), /not a block/);
});
