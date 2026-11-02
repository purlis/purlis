// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What may run on a push to main (#4471): `ci.yml` itself is read at the end of this file, so a
// new job that could stop landing and releases fails `web (types, lint, format, test)`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  LISTED,
  REQUIRED_CHECKS,
  REQUIRED_ELSEWHERE,
  evaluate,
  jobsOf,
  onPush,
  requiredDrift,
  sortJobs,
} from "./ci-push-jobs.mjs";

const SCRIPT = fileURLToPath(new URL("./ci-push-jobs.mjs", import.meta.url));
const CI = fileURLToPath(new URL("../.github/workflows/ci.yml", import.meta.url));

/** A workflow of the jobs `body` writes, each already indented as `jobs:` holds it. */
const workflow = (body) => `name: ci\non:\n  push:\njobs:\n${body}`;

const gate = `  gate:
    name: the gate
    needs: [unit]
    if: \${{ !cancelled() }}
    runs-on: ubuntu-24.04
    steps:
      - run: test "\${{ needs.unit.result }}" = success
`;
const unit = `  unit:
    name: unit tests
    runs-on: ubuntu-24.04
    steps:
      - run: true
`;

const sort = (body, listed = {}) => sortJobs(workflow(body), { required: ["the gate"], listed });
const refusedIds = (body, listed) => sort(body, listed).refused.map((job) => job.id);

test("an expression is read as a push to main reads it, and what it cannot know stays unknown", () => {
  assert.equal(evaluate("github.event_name == 'push'"), true);
  assert.equal(evaluate("github.event_name == 'schedule'"), false);
  assert.equal(evaluate("github.event_name != 'pull_request' || needs.changes.outputs.a != 'false'"), true);
  assert.equal(evaluate("github.event_name == 'push' && github.ref == 'refs/heads/main'"), true);
  assert.equal(evaluate("!cancelled() && github.event_name != 'schedule'"), true);
  assert.equal(
    evaluate(
      "!cancelled() && (github.event_name == 'pull_request' || github.event_name == 'merge_group') && needs.a.outputs.b != 'false'",
    ),
    false,
  );
  assert.equal(evaluate("${{ github.event_name == 'push' }}"), true);
  assert.equal(evaluate("needs.changes.outputs.rust != 'false'"), undefined);
  assert.equal(evaluate("contains(github.ref, 'main')"), undefined);
  assert.equal(evaluate("matrix.evidence"), undefined);
  // Unknown never turns into false: only a known false can take a job off a push.
  assert.equal(evaluate("!matrix.evidence"), undefined);
  assert.equal(evaluate("matrix.a || github.event_name == 'push'"), true);
  assert.equal(evaluate("matrix.a && github.event_name == 'schedule'"), false);
  assert.equal(evaluate("this is ( not an expression"), undefined);
});

test("whether a job runs on a push is its `if:`, and a job with none does", () => {
  assert.equal(onPush({ if: undefined }), true);
  assert.equal(onPush({ if: "github.event_name == 'schedule'" }), false);
  assert.equal(onPush({ if: "${{ !cancelled() && needs.changes.outputs.rust != 'false' }}" }), true);
});

test("jobs are read with their name, what they need, their `if:` and their job-level continue-on-error", () => {
  const [one, two, three] = jobsOf(
    workflow(`  # a comment
  one:
    name: "first"
    needs: changes
    if: github.event_name == 'push'
    continue-on-error: true
    runs-on: x
    steps:
      # needs.nothing.result is a comment
      - run: echo \${{ needs.changes.result }}
        continue-on-error: false
  two:
    needs: [a, b]
    if: >-
      github.event_name == 'push' &&
      needs.a.result == 'success'
    steps:
      - run: true
        continue-on-error: true
  three:
    needs:
      - a
      - b
`),
  );
  assert.deepEqual(
    [one.id, one.name, one.needs, one.if, one.continueOnError],
    ["one", "first", ["changes"], "github.event_name == 'push'", "true"],
  );
  assert.deepEqual(one.reads, ["changes"]);
  assert.deepEqual(two.needs, ["a", "b"]);
  assert.equal(two.if, "github.event_name == 'push' && needs.a.result == 'success'");
  assert.equal(two.continueOnError, undefined, "a step's continue-on-error is not the job's");
  assert.deepEqual(three.needs, ["a", "b"]);
});

test("a workflow with no jobs to read is refused rather than passed", () => {
  assert.throws(() => jobsOf("name: ci\n"), /no jobs/);
});

test("a job that runs on push and nothing requires is refused, and told what to choose", () => {
  const { refused } = sort(gate + unit + "  extra:\n    runs-on: x\n");
  assert.deepEqual(
    refused.map((job) => job.id),
    ["extra"],
  );
  for (const word of ["land.mjs", "release.yml", "required", "continue-on-error", "LISTED"]) {
    assert.ok(refused[0].why.includes(word), `${word} in: ${refused[0].why}`);
  }
});

test("a job a required job reads the result of passes, however far behind it", () => {
  assert.deepEqual(refusedIds(gate + unit), []);
  const far = `  part:
    runs-on: x
  unit:
    needs: part
    steps:
      - run: test "\${{ needs.part.result }}" = success
`;
  assert.deepEqual(refusedIds(gate + far), []);
});

test("needing a job is not reaching it: a required job that only needs it passes when it fails", () => {
  // With `!cancelled()` the gate runs and passes. With no `if:` it is skipped, and a skipped
  // required check counts as passing (`rust-checks` in ci.yml says so).
  const blind = `  gate:
    name: the gate
    needs: [unit]
    if: \${{ !cancelled() }}
    steps:
      - run: true
`;
  const skipped = `  gate:
    name: the gate
    needs: [unit]
    steps:
      - run: true
`;
  assert.deepEqual(refusedIds(blind + unit), ["unit"]);
  assert.deepEqual(refusedIds(skipped + unit), ["unit"]);
  const reader = `  gate:
    name: the gate
    needs: [unit]
    if: \${{ !cancelled() && !contains(needs.*.result, 'failure') }}
    steps:
      - run: true
`;
  assert.deepEqual(refusedIds(reader + unit), []);
});

test("a job behind a job nothing requires is not reached through it", () => {
  const behind = `  loose:
    needs: unit
    steps:
      - run: test "\${{ needs.unit.result }}" = success
`;
  assert.deepEqual(refusedIds(gate.replace("the gate", "not required") + unit + behind), [
    "gate",
    "unit",
    "loose",
    "the gate",
  ]);
});

test("evidence passes only when continue-on-error is true on a push", () => {
  const job = (value) => `  extra:\n    continue-on-error: ${value}\n    runs-on: x\n`;
  assert.deepEqual(refusedIds(gate + unit + job("true")), []);
  assert.deepEqual(refusedIds(gate + unit + job("${{ github.event_name == 'push' }}")), []);
  assert.deepEqual(refusedIds(gate + unit + job("${{ github.event_name == 'schedule' }}")), ["extra"]);
  assert.deepEqual(refusedIds(gate + unit + job("${{ matrix.evidence }}")), ["extra"]);
  assert.deepEqual(refusedIds(gate + unit + job("false")), ["extra"]);
});

test("a job that never runs on a push has nothing to choose", () => {
  assert.deepEqual(refusedIds(gate + unit + "  nightly:\n    if: github.event_name == 'schedule'\n"), []);
});

test("a job whose `if:` cannot be read counts as running on a push", () => {
  assert.deepEqual(refusedIds(gate + unit + "  odd:\n    if: matrix.a == 'b'\n"), ["odd"]);
});

test("a listed job passes with its reason", () => {
  const sorted = sort(gate + unit + "  extra:\n    runs-on: x\n", { extra: "a person acts on it" });
  assert.deepEqual(sorted.refused, []);
  assert.deepEqual(
    sorted.listed.map((job) => [job.id, job.reason]),
    [["extra", "a person acts on it"]],
  );
});

test("a listing with no reason, or that no longer does anything, is refused", () => {
  assert.deepEqual(refusedIds(gate + unit + "  extra:\n    runs-on: x\n", { extra: " " }), ["extra"]);
  assert.deepEqual(refusedIds(gate + unit, { gone: "it left" }), ["gone"]);
  assert.deepEqual(refusedIds(gate + unit, { unit: "it is required now" }), ["unit"]);
});

test("a required check that no job posts is refused: renaming a job must not end its gating", () => {
  const { refused } = sortJobs(workflow(unit), { required: ["the gate"], listed: {} });
  assert.deepEqual(
    refused.map((job) => job.id),
    ["unit", "the gate"],
  );
  assert.match(refused[1].why, /no job/);
});

test("a required name matches a job whose name is written with the matrix", () => {
  const matrixed = "  app:\n    name: app builds (${{ matrix.os }}${{ matrix.evidence && ', x' || '' }})\n";
  const { required, refused } = sortJobs(workflow(matrixed), {
    required: ["app builds (macos-latest)", "app builds (ubuntu-24.04)"],
    listed: {},
  });
  assert.deepEqual(refused, []);
  assert.deepEqual(
    required.map((job) => job.id),
    ["app"],
  );
});

test("what main requires and these lists disagree on is named both ways", () => {
  const live = [...REQUIRED_CHECKS, ...REQUIRED_ELSEWHERE];
  assert.deepEqual(requiredDrift(live), { missing: [], extra: [] });
  assert.deepEqual(requiredDrift(["new check", ...live.slice(1)]), {
    missing: ["new check"],
    extra: [live[0]],
  });
});

const run = (args) => spawnSync("node", [SCRIPT, ...args], { encoding: "utf8" });

test("the command names every job's standing, and fails on a refused one", () => {
  const dir = mkdtempSync(join(tmpdir(), "ci-push-jobs-"));
  try {
    const good = run([CI]);
    assert.equal(good.status, 0, good.stdout + good.stderr);
    assert.match(good.stdout, /listed {2}parity/);
    const bad = join(dir, "ci.yml");
    writeFileSync(bad, `${readFileSync(CI, "utf8").trimEnd()}\n\n  extra:\n    runs-on: x\n    steps:\n      - run: false\n`);
    const refused = run([bad]);
    assert.equal(refused.status, 1);
    assert.match(refused.stderr, /"extra"/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// `ci.yml` as it stands: this is the check itself.
test("every job of ci.yml that runs on a push is required, evidence or listed", () => {
  const { refused } = sortJobs(readFileSync(CI, "utf8"), { required: REQUIRED_CHECKS, listed: LISTED });
  assert.deepEqual(
    refused.map((job) => job.why),
    [],
  );
});

test("ci.yml's standings are the ones this ticket read", () => {
  const sorted = sortJobs(readFileSync(CI, "utf8"), { required: REQUIRED_CHECKS, listed: LISTED });
  const ids = (list) => list.map((job) => job.id).sort();
  assert.deepEqual(ids(sorted.listed), Object.keys(LISTED).sort());
  assert.ok(ids(sorted.evidence).includes("outcome-bars"));
  assert.ok(ids(sorted.reached).includes("rust"));
  assert.ok(ids(sorted.absent).includes("fedora"));
});

test("the tools' tests, this one among them, run in a job a required check reads", () => {
  const jobs = jobsOf(readFileSync(CI, "utf8"));
  const web = jobs.find((job) => job.id === "web");
  assert.ok(web.text.includes('node --test "../tools/*.test.mjs"'));
  assert.ok(sortJobs(readFileSync(CI, "utf8"), { required: REQUIRED_CHECKS, listed: LISTED }).reached.some(({ id }) => id === "web"));
});
