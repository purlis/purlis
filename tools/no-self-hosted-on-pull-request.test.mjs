// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// No workflow a pull request can start runs on a self-hosted runner. Run with `node --test tools/`.
//
// This repository is public, so anyone can open a pull request, and a workflow that runs on one
// runs code of the pull request's author. On a GitHub-hosted runner that is a machine thrown away
// afterwards; on a self-hosted Mac it is the operator's own machine, persistent, with a keychain.
// `release.yml` may name one (through `vars.MACOS_RELEASE_RUNNER`, docs/updating.md) because only a
// push to `main`, a tag, the schedule or a run by hand starts it; a workflow that a pull request
// starts may not, and that is held here by what its text says, so a later edit cannot add one.
//
// Reachable from a pull request: a workflow with any trigger in REACHABLE, `workflow_call` among
// them, which runs as its caller does. Refused in such a workflow: the text `self-hosted` anywhere
// outside a comment (a matrix of labels included), a runner `group:`, and any `runs-on` that is not
// a literal or `${{ matrix.<key> }}`: a variable, an input, a secret, a `needs` output or an `env`
// could each send a pull request to one.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";

const dir = new URL("../.github/workflows/", import.meta.url);
// Every trigger a pull request's author (or a comment, a review, a queue entry or a run of another
// workflow that one started) can use to make code of theirs run.
const REACHABLE = [
  "pull_request",
  "pull_request_target",
  "pull_request_review",
  "pull_request_review_comment",
  "issue_comment",
  "merge_group",
  "workflow_run",
  "workflow_call",
];

/** The top-level `on:` keys of a workflow's text, in any of YAML's three spellings. */
export function triggers(text) {
  const lines = text.split("\n");
  const at = lines.findIndex((l) => /^["']?on["']?:/.test(l));
  if (at === -1) return [];
  const first = lines[at].replace(/^["']?on["']?:/, "").replace(/#.*$/, "").trim();
  if (first) return first.replace(/[[\]]/g, "").split(",").map((k) => k.trim()).filter(Boolean);
  const keys = [];
  let indent = null;
  for (const line of lines.slice(at + 1)) {
    if (line.trim() === "" || line.trim().startsWith("#")) continue;
    if (!/^\s/.test(line)) break;
    const depth = line.length - line.trimStart().length;
    indent ??= depth;
    if (depth !== indent) continue;
    const key = /^\s+([\w-]+):/.exec(line) ?? /^\s+- *([\w-]+)/.exec(line);
    if (key) keys.push(key[1]);
  }
  assert.ok(keys.length > 0, "`on:` was found and no trigger could be read from it");
  return keys;
}

/** What each `runs-on:` of a workflow says: the value, and the lines under it when it is a block. */
export function runsOn(text) {
  const lines = text.split("\n");
  const out = [];
  lines.forEach((line, i) => {
    const m = /^(\s*)runs-on:(.*)$/.exec(line);
    if (!m || line.trim().startsWith("#")) return;
    let said = m[2].replace(/\s+#.*$/, "").trim();
    for (const next of lines.slice(i + 1)) {
      if (next.trim() === "" || next.trim().startsWith("#")) continue;
      if (next.length - next.trimStart().length <= m[1].length) break;
      said += ` ${next.trim()}`;
    }
    out.push(said);
  });
  return out;
}

/** The refusals a workflow's text earns if a pull request can start it. */
export function refusals(text) {
  if (!triggers(text).some((t) => REACHABLE.includes(t))) return [];
  const code = text.split("\n").filter((l) => !l.trim().startsWith("#")).join("\n");
  const found = runsOn(text).filter((said) => {
    if (/self-hosted|\bgroup:/i.test(said)) return true;
    return /\$\{\{/.test(said) && !/^\$\{\{\s*matrix\.[\w-]+\s*\}\}$/.test(said);
  });
  // `--deny-self-hosted-runners` is attestation verification's flag, not a label.
  if (/(?<!deny-)self-hosted/i.test(code) && found.length === 0) found.push("self-hosted (outside runs-on, e.g. a matrix)");
  return found;
}

test("no workflow a pull request can start runs on a self-hosted runner", () => {
  const names = readdirSync(dir).filter((n) => n.endsWith(".yml"));
  assert.ok(names.length > 10, "the workflows were not found");
  const bad = {};
  for (const name of names) {
    const found = refusals(readFileSync(new URL(name, dir), "utf8"));
    if (found.length) bad[name] = found;
  }
  assert.deepEqual(bad, {});
});

test("the guard sees a self-hosted label however a pull request workflow spells it", () => {
  const on = (head, runner) => `name: x\n${head}\njobs:\n  a:\n    ${runner}\n    steps: []\n`;
  const pr = "on:\n  pull_request:\n    branches: [main]";
  assert.deepEqual(refusals(on(pr, "runs-on: self-hosted")), ["self-hosted"]);
  assert.deepEqual(refusals(on(pr, "runs-on: [self-hosted, macOS]")), ["[self-hosted, macOS]"]);
  assert.equal(refusals(on("on: [push, pull_request]", "runs-on: [self-hosted]")).length, 1);
  assert.equal(refusals(on("on: pull_request_target", "runs-on: self-hosted")).length, 1);
  assert.equal(refusals(on("on:\n  workflow_call:", "runs-on: self-hosted")).length, 1);
  assert.equal(refusals(on(pr, "runs-on:\n      - self-hosted\n      - macOS")).length, 1);
  assert.equal(refusals(on(pr, "runs-on:\n      group: mac")).length, 1);
  assert.equal(refusals(on(pr, "runs-on: ${{ fromJSON(vars.MACOS_RELEASE_RUNNER || '\"macos-latest\"') }}")).length, 1);
  assert.equal(refusals(on(pr, "runs-on: ${{ matrix.os }}")).length, 0);
  // The route a literal check misses: the label sits in a matrix, or comes out of another job.
  const matrix = "runs-on: ${{ matrix.os }}\n    strategy:\n      matrix:\n        os: [self-hosted, macOS]";
  assert.equal(refusals(on(pr, matrix)).length, 1);
  assert.equal(refusals(on(pr, "runs-on: ${{ needs.plan.outputs.runner }}")).length, 1);
  assert.equal(refusals(on(pr, "runs-on: ${{ env.RUNNER }}")).length, 1);
  for (const trigger of ["merge_group", "workflow_run", "issue_comment", "pull_request_review"]) {
    assert.equal(refusals(on(`on:\n  ${trigger}:`, "runs-on: self-hosted")).length, 1, trigger);
  }
  assert.equal(refusals(on("on:\n    pull_request:", "runs-on: self-hosted")).length, 1, "four-space indent");
});

test("the guard leaves alone a workflow no pull request can start, and a hosted runner", () => {
  const on = (head, runner) => `name: x\n${head}\njobs:\n  a:\n    ${runner}\n    steps: []\n`;
  assert.deepEqual(refusals(on("on:\n  push:\n    branches: [main]\n  workflow_dispatch:", "runs-on: self-hosted")), []);
  assert.deepEqual(refusals(on("on:\n  pull_request:", "runs-on: ubuntu-24.04")), []);
});

test("release.yml, the one workflow that may name a self-hosted runner, cannot be started by a pull request", () => {
  const found = triggers(readFileSync(new URL("release.yml", dir), "utf8"));
  assert.deepEqual(found.sort(), ["push", "schedule", "workflow_dispatch"]);
});
