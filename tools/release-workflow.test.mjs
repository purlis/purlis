// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `release.yml`'s dev builds queue, and a run that publishes nothing never takes a build's
// place in the queue. Run with `node --test tools/`.
//
// On 2026-10-13 two dev builds were lost in one afternoon (runs 651 and 655): each was waiting
// behind the build in progress when a run that a red or cancelled `ci` had started took the
// group's one pending place, cancelled it, and then skipped. And run 654 published a commit
// nine commits older than the one the channel served. These hold the trigger, the group and
// the gate to the shape that stops both, by working out what the file's own expressions
// answer for each kind of event, so a later edit that words them differently still passes and
// one that changes what they answer does not.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { publishedBuild } from "./dev-build-order.mjs";

const REPO = "purlis/purlis";
const text = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
const lines = text.split("\n");

/** The lines under the top-level `key:`, comments and blank lines dropped. */
function block(key) {
  const start = lines.indexOf(`${key}:`);
  assert.notEqual(start, -1, `release.yml has no top-level \`${key}:\``);
  const body = [];
  for (const line of lines.slice(start + 1)) {
    if (line.trim() === "" || line.trim().startsWith("#")) continue;
    if (!line.startsWith(" ")) break;
    body.push(line);
  }
  return body;
}

/** The lines of the job `name`, comments dropped. */
function job(name) {
  const jobs = block("jobs");
  const start = jobs.indexOf(`  ${name}:`);
  assert.notEqual(start, -1, `release.yml has no \`${name}\` job`);
  const end = jobs.findIndex((line, i) => i > start && /^ {2}\S/.test(line));
  return jobs.slice(start + 1, end === -1 ? undefined : end);
}

/** The value written after `key:` on one line of `body`, at `indent` spaces. */
function value(body, indent, key) {
  const prefix = `${" ".repeat(indent)}${key}:`;
  const found = body.filter((line) => line.startsWith(prefix));
  assert.equal(found.length, 1, `expected one \`${key}:\` at ${indent} spaces: ${found}`);
  return found[0].slice(prefix.length).trim();
}

/**
 * What a workflow expression answers for `github`.
 *
 * Only what these two expressions use: `==`, `!=`, `&&`, `||`, brackets, single-quoted
 * strings, `format` and paths into `github`. `&&` and `||` answer one of their operands, as
 * JavaScript's do, and a path that leads nowhere is null. Anything else in the expression
 * fails the test: it has to be taught here before the file may use it.
 */
function evaluate(expression, github) {
  const parts = expression.split(/('(?:[^']|'')*')/);
  const script = parts
    .map((part, i) => {
      if (i % 2 === 1) return JSON.stringify(part.slice(1, -1).replaceAll("''", "'"));
      assert.match(part, /^[\w\s.()!=&|,]*$/, `an expression these tests cannot read: ${part}`);
      return part
        .replace(/==/g, "===")
        .replace(/!=(?!=)/g, "!==")
        .replace(/\bgithub((?:\.\w+)+)/g, (_, path) => `(github${path.replaceAll(".", "?.")} ?? null)`);
    })
    .join("");
  const format = (template, ...args) => template.replace(/\{(\d+)\}/g, (_, n) => String(args[n]));
  return new Function("github", "format", `"use strict"; return (${script});`)(github, format);
}

/** A `github` context for a `release` run that a `ci` run started. */
function afterCi({ conclusion, event = "push", from = REPO, branch = "main", runId }) {
  return {
    event_name: "workflow_run",
    repository: REPO,
    ref: "refs/heads/main",
    run_id: runId,
    event: {
      workflow_run: {
        conclusion,
        event,
        head_branch: branch,
        head_sha: "8781c167".padEnd(40, "0"),
        head_repository: { full_name: from },
      },
    },
  };
}

/** Every kind of `ci` run on a branch called `main` whose end starts a `release` run. */
const CI_ENDS = [
  { name: "a green ci of a push to main", ci: { conclusion: "success" }, publishes: true },
  { name: "a red ci of a push to main", ci: { conclusion: "failure" }, publishes: false },
  { name: "a cancelled ci of a push to main", ci: { conclusion: "cancelled" }, publishes: false },
  { name: "a ci that timed out", ci: { conclusion: "timed_out" }, publishes: false },
  {
    name: "the nightly ci, green",
    ci: { conclusion: "success", event: "schedule" },
    publishes: false,
  },
  {
    name: "a green ci of a pull request from a fork's branch called main",
    ci: { conclusion: "success", event: "pull_request", from: "stranger/purlis" },
    publishes: false,
  },
  {
    name: "a green ci of a push in another repository",
    ci: { conclusion: "success", from: "stranger/purlis" },
    publishes: false,
  },
];

const concurrency = block("concurrency");
const group = () => {
  const written = value(concurrency, 2, "group");
  const found = /^([\w-]*)\$\{\{ (.*) \}\}$/.exec(written);
  assert.ok(found, `the group is one prefix and one expression: ${written}`);
  return (github) => `${found[1]}${evaluate(found[2], github)}`;
};

/** `plan`'s `if:`, a folded scalar, as one expression. */
function planCondition() {
  const body = job("plan");
  const start = body.indexOf("    if: >-");
  assert.notEqual(start, -1, "`plan` has no folded `if:`");
  const folded = [];
  for (const line of body.slice(start + 1)) {
    if (!line.startsWith("      ")) break;
    folded.push(line.trim());
  }
  return folded.join(" ");
}

test("a release run is started by the end of ci on main, and by nothing else that ends", () => {
  const on = block("on");
  const keys = on.filter((line) => /^ {2}\S/.test(line)).map((line) => line.trim());
  assert.deepEqual(keys, ["workflow_dispatch:", "push:", "schedule:", "workflow_run:"]);
  const run = on.slice(on.indexOf("  workflow_run:") + 1);
  assert.deepEqual(run, ["    workflows: [ci]", "    types: [completed]", "    branches: [main]"]);
});

test("a build in progress is never cancelled", () => {
  assert.equal(value(concurrency, 2, "cancel-in-progress"), "false");
  assert.deepEqual(
    concurrency.map((line) => line.trim().split(":")[0]),
    ["group", "cancel-in-progress"],
  );
  // And no job takes a group of its own that could cancel one.
  assert.deepEqual(
    block("jobs").filter((line) => /^\s+(concurrency|cancel-in-progress):/.test(line)),
    [],
  );
});

test("plan runs for a green ci of a push to this repository's main, and for no other ci", () => {
  const condition = planCondition();
  for (const { name, ci, publishes } of CI_ENDS) {
    assert.equal(evaluate(condition, afterCi({ ...ci, runId: 1 })), publishes, name);
  }
  // The other three ways in are not `workflow_run`s, and `plan` runs for each.
  for (const event_name of ["push", "schedule", "workflow_dispatch"]) {
    assert.equal(evaluate(condition, { event_name, repository: REPO, event: {} }), true);
  }
});

test("only a run that plan lets publish waits in the dev builds' group", () => {
  const groupOf = group();
  const condition = planCondition();
  const dev = groupOf(afterCi({ conclusion: "success", runId: 1 }));
  assert.equal(dev, "release-main");
  for (const { name, ci } of CI_ENDS) {
    const github = afterCi({ ...ci, runId: 7 });
    assert.equal(groupOf(github) === dev, evaluate(condition, github), name);
  }
});

test("a run that publishes nothing has a group no other run shares", () => {
  const groupOf = group();
  const others = new Set([
    groupOf(afterCi({ conclusion: "success", runId: 1 })),
    groupOf({ event_name: "schedule", repository: REPO, ref: "refs/heads/main", event: {} }),
    groupOf({ event_name: "push", repository: REPO, ref: "refs/tags/v1.2.3", event: {} }),
    groupOf({ event_name: "workflow_dispatch", repository: REPO, ref: "refs/heads/main", event: {} }),
  ]);
  for (const { name, ci, publishes } of CI_ENDS) {
    if (publishes) continue;
    const [first, second] = [7, 8].map((runId) => groupOf(afterCi({ ...ci, runId })));
    assert.notEqual(first, second, `${name}: two such runs share a group`);
    assert.ok(!others.has(first), `${name}: shares ${first} with a run that builds`);
    assert.match(first, /^release-nothing-7$/, name);
  }
});

test("the profile guard, a tag and a build started by hand keep their own groups", () => {
  const groupOf = group();
  const at = (event_name, ref) => groupOf({ event_name, repository: REPO, ref, run_id: 3, event: {} });
  assert.equal(at("schedule", "refs/heads/main"), "release-profile-guard");
  assert.equal(at("push", "refs/tags/v1.2.3"), "release-refs/tags/v1.2.3");
  assert.equal(at("workflow_dispatch", "refs/heads/try"), "release-refs/heads/try");
});

test("a dev build is the commit its green ci ran on, and no other", () => {
  const plan = job("plan");
  const sha = "${{ github.event.workflow_run.head_sha || github.sha }}";
  // The first checkout of `plan`, and the `SHA` every later job checks out as `plan`'s `sha`.
  assert.equal(plan.find((line) => line.trim().startsWith("ref:"))?.trim(), `ref: ${sha}`);
  assert.ok(plan.includes(`          SHA: ${sha}`), "plan's SHA is not the ci run's commit");
  assert.ok(plan.includes('            echo "sha=$SHA"; echo "tag=$tag"; echo "signed=$HAS_UPDATER_KEY"; echo "skip=$skip"'));
  for (const name of ["build", "sbom", "manifest", "publish"]) {
    const refs = job(name).filter((line) => line.trim().startsWith("ref:"));
    assert.deepEqual(
      refs.map((line) => line.trim()),
      ["ref: ${{ needs.plan.outputs.sha }}"],
      name,
    );
  }
});

test("plan asks whether the dev channel already serves this commit or a later one", () => {
  const plan = job("plan").map((line) => line.trim());
  const ask = plan.indexOf(
    'order=$(node tools/dev-build-order.mjs --version "$version" --sha "$SHA")',
  );
  assert.notEqual(ask, -1, "plan does not ask tools/dev-build-order.mjs");
  assert.equal(plan[ask - 1], 'if [ "$EVENT" = workflow_run ] && [ "$skip" != "true" ]; then');
  assert.deepEqual(plan.slice(ask + 1, ask + 8), [
    'case "$order" in',
    'publish:*) echo "$order" ;;',
    "skip:*)",
    'echo "::notice::not built, and nothing is published: ${order#skip: }"',
    "skip=true; publish=false ;;",
    '*) echo "::error::tools/dev-build-order.mjs answered neither publish nor skip: $order"; exit 1 ;;',
    "esac",
  ]);
  // Asked before the outputs are written, in a script that stops at the first failure.
  assert.ok(ask < plan.indexOf('} >> "$GITHUB_OUTPUT"'));
  assert.ok(plan.includes("GH_TOKEN: ${{ github.token }}"));
  // And `skip` is what keeps every build, and so every publish, from starting.
  assert.equal(value(job("build"), 4, "if"), "needs.plan.outputs.skip != 'true'");
  assert.equal(value(job("publish"), 4, "needs"), "[plan, build, manifest, provenance]");
});

test("the publish job asks again before it replaces the first dev asset, and stops unless told to publish", () => {
  // "Re-run failed jobs" on an old run keeps `plan`'s answer and runs only `publish`.
  const publish = job("publish").map((line) => line.trim());
  const step = publish.indexOf("- name: Dev — replace the assets of the one `dev` prerelease");
  assert.notEqual(step, -1, "publish has no dev step");
  const ask = publish.indexOf(
    'order=$(node tools/dev-build-order.mjs --version "$VERSION" --sha "$SHA")',
  );
  assert.ok(ask > step, "the dev step does not ask tools/dev-build-order.mjs");
  assert.deepEqual(publish.slice(ask + 1, ask + 5), [
    'case "$order" in',
    'publish:*) echo "$order" ;;',
    '*) echo "::error::not published, and no asset was replaced: ${order#skip: }"; exit 1 ;;',
    "esac",
  ]);
  assert.ok(publish.slice(step, ask).includes("set -euo pipefail"));
  // Nothing of the dev release is written, replaced or deleted before the answer.
  const writes = publish
    .map((line, i) => [line, i])
    .filter(([line, i]) => i > step && /replace-release-assets\.sh dev|gh release edit dev|-X DELETE/.test(line));
  assert.ok(writes.length >= 4, `the dev step's writes were not found: ${writes}`);
  for (const [line, i] of writes) assert.ok(i > ask, `written before the order is asked: ${line}`);
  // The job has what the tool needs: the token, and the version and commit `plan` gave.
  for (const want of [
    "GH_TOKEN: ${{ github.token }}",
    "VERSION: ${{ needs.plan.outputs.version }}",
    "SHA: ${{ needs.plan.outputs.sha }}",
  ]) {
    assert.ok(publish.includes(want), `publish lacks ${want}`);
  }
});

test("every job has a deadline, so a hung one cannot hold the dev builds' group for six hours", () => {
  const names = block("jobs")
    .filter((line) => /^ {2}\S/.test(line))
    .map((line) => line.trim().slice(0, -1));
  assert.deepEqual(names, ["plan", "build", "sbom", "manifest", "provenance", "publish", "profile-guard"]);
  let whole = 0;
  for (const name of names) {
    const minutes = Number(value(job(name), 4, "timeout-minutes"));
    assert.ok(Number.isInteger(minutes) && minutes >= 10 && minutes <= 90, `${name}: ${minutes}`);
    whole += minutes;
  }
  // The longest a run can hold the group, every job at its deadline one after another.
  assert.ok(whole <= 240, `a run may hold the group for ${whole} minutes`);
});

test("the tool reads the commit from the notes the manifest job writes, and the version plan gives", () => {
  const manifest = job("manifest").map((line) => line.trim());
  const notes = 'notes="charter $VERSION, a dev build of $SHA."';
  assert.ok(manifest.includes(notes), `the manifest job no longer writes ${notes}`);
  const sha = "0854cd21".padEnd(40, "f");
  const written = notes
    .slice('notes="'.length, -1)
    .replace("$VERSION", "0.4.2-dev.663")
    .replace("$SHA", sha);
  assert.deepEqual(publishedBuild(JSON.stringify({ version: "0.4.2-dev.663", notes: written })), {
    version: "0.4.2-dev.663",
    sha,
  });
  // The number only grows, and runs in one group publish in the order they were numbered.
  const plan = job("plan").map((line) => line.trim());
  assert.ok(plan.includes('tag=""; version="$base-dev.$GITHUB_RUN_NUMBER"'));
});

test("the runs list names the commit a run builds, not main's head when ci ended", () => {
  const name = /^\$\{\{ (.*) \}\}$/.exec(value(lines, 0, "run-name"));
  assert.ok(name, "run-name is one expression");
  const github = afterCi({ conclusion: "success", runId: 1 });
  assert.equal(evaluate(name[1], github), `ci success (push) of ${github.event.workflow_run.head_sha}`);
  assert.equal(
    evaluate(name[1], { event_name: "push", ref_name: "v1.2.3", event: {} }),
    "stable v1.2.3",
  );
  assert.equal(evaluate(name[1], { event_name: "schedule", event: {} }), "profile guard");
  assert.equal(
    evaluate(name[1], { event_name: "workflow_dispatch", ref_name: "try", event: {} }),
    "on demand, try",
  );
});
