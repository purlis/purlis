// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `budget-run.yml`'s shape (#7737, spec #7730): the budget run measures on the benchmark Mac alone,
// from no trigger a pull request can reach, one run at a time with none cancelled once started,
// with `release.yml`'s steps for a persistent runner and its light checkout, in both renderer
// arms. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const text = readFileSync(new URL("../.github/workflows/budget-run.yml", import.meta.url), "utf8");

/** The workflow's jobs, each as its own lines, by name: the two-space keys under `jobs:`. */
function jobsOf(yml) {
  const jobs = {};
  let current = null;
  let inJobs = false;
  for (const line of yml.split("\n")) {
    if (/^\S/.test(line)) {
      inJobs = line.startsWith("jobs:");
      current = null;
      continue;
    }
    const key = inJobs && line.match(/^ {2}([\w-]+):\s*$/);
    if (key) jobs[(current = key[1])] = [];
    else if (current) jobs[current].push(line);
  }
  return jobs;
}

/** The steps of the list after `header`, each as its lines, comments left out. */
function stepsAfter(yml, header) {
  const lines = yml.split("\n");
  const at = lines.findIndex((line) => header.test(line));
  assert.ok(at >= 0, `no ${header} line`);
  const steps = [];
  for (const line of lines.slice(at + 1)) {
    if (line !== "" && !line.startsWith("      ")) break;
    if (line.startsWith("      - ")) steps.push([]);
    if (steps.length > 0 && line.trim() !== "" && !/^\s*#/.test(line)) steps.at(-1).push(line);
  }
  return steps.map((step) => step.join("\n"));
}

const code = (lines) => lines.filter((l) => !/^\s*#/.test(l));
const jobs = jobsOf(text);
const mac = code(jobs["budget-run"]).join("\n");
const macSteps = () => {
  const lines = text.split("\n");
  const from = lines.findIndex((l) => /^ {2}budget-run:\s*$/.test(l));
  const to = lines.findIndex((l, i) => i > from && /^ {2}[\w-]+:\s*$/.test(l));
  return stepsAfter(lines.slice(from, to).join("\n"), /^ {4}steps:$/);
};
const named = (name) => macSteps().find((step) => step.includes(`- name: ${name}`));

test("it starts on main's push and nightly, and on nothing else", () => {
  // The whole `on:` block, comments aside: no other trigger, so no pull request reaches it
  // (`tools/no-self-hosted-on-pull-request.test.mjs` holds that too).
  const on = /\non:\n((?:[ #].*\n|\n)+?)(?=\S)/.exec(text)?.[1] ?? "";
  assert.deepEqual(code(on.split("\n")).filter((l) => l.trim() !== ""), [
    "  push:",
    "    branches: [main]",
    "  schedule:",
    '    - cron: "17 3 * * *"',
  ]);
});

test("the Mac job's runner is BUDGET_RUN_RUNNER, with no fallback, and it runs only on a self-hosted one", () => {
  const runner = /^ {4}runs-on: (.*)$/m.exec(mac)?.[1];
  assert.equal(runner, "${{ fromJSON(vars.BUDGET_RUN_RUNNER) }}");
  assert.doesNotMatch(runner, /\|\||macos-|ubuntu-/);
  // Unset, or set to a hosted label, the job is skipped: it never queues on GitHub's runners.
  assert.match(mac, /^ {4}if: contains\(vars\.BUDGET_RUN_RUNNER, 'self-hosted'\)$/m);
});

test("one run at a time and the newest pending push, none cancelled once started, and the nightly queues", () => {
  const block = /\nconcurrency:\n((?: {2}.*\n|\s*\n)+)/.exec(text)?.[1] ?? "";
  const lines = code(block.split("\n")).join("\n");
  assert.match(lines, /cancel-in-progress: false/);
  // Grouped by the event, never by the commit: one group for main's pushes, one for the nightly.
  assert.match(lines, /group: budget-run-\$\{\{ github\.event_name == 'schedule' && 'nightly' \|\| 'main' \}\}/);
  assert.doesNotMatch(lines, /github\.sha|github\.run_id/);
  assert.doesNotMatch(text, /cancel-in-progress: true/);
});

test("the Mac job has release.yml's steps for a persistent runner, each only where it is self-hosted", () => {
  for (const name of [
    "A self-hosted runner's workspace starts empty",
    "A self-hosted runner's Cargo, rustup and temporary folder are this job's own",
    "A self-hosted Mac's window can be painted",
  ]) {
    const step = named(name);
    assert.ok(step, `no step ${name}`);
    assert.match(step, /if: runner\.environment == 'self-hosted'/, name);
  }
  assert.match(named("Say that these rows ran on the benchmark Mac"), /GITHUB_STEP_SUMMARY/);
  assert.match(named("A self-hosted runner keeps no job folder"), /if: always\(\) && runner\.environment == 'self-hosted'/);
  assert.match(named("A self-hosted runner keeps no job folder"), /rm -rf "\$RUNNER_TEMP\/job-home"/);
  // The workspace is emptied before the checkout, and the job's homes set before the toolchain.
  const order = macSteps().map((s) => /^ {6}- (?:name|uses): (.*)$/m.exec(s)?.[1] ?? s);
  const at = (what) => order.findIndex((n) => n.startsWith(what));
  assert.ok(at("A self-hosted runner's workspace starts empty") < at("actions/checkout"));
  assert.ok(at("A self-hosted runner's Cargo") < at("./.github/actions/rust-toolchain"));
  assert.equal(at("A self-hosted runner keeps no job folder"), order.length - 1);
});

test("the Mac job's checkout is the light one, without the story baselines", () => {
  const checkout = macSteps().find((s) => s.includes("actions/checkout@"));
  assert.match(checkout, /filter: blob:none/);
  assert.match(checkout, /sparse-checkout-cone-mode: false/);
  assert.match(checkout, /!\*\*\/__screenshots__\//);
  assert.match(checkout, /persist-credentials: false/);
});

test("it measures L2 with the window bench, in both renderer arms, on the stand-in harness's build", () => {
  const bench = named("L2, a tab switch, in both renderer arms");
  assert.match(bench, /node tools\/bench\.mjs --skip-build --only window --arms dom,webgl/);
  assert.match(bench, /--specs burst\.bench\.ts --grep "tab switch"/);
  assert.match(bench, /--target-dir target\/dev-release/);
  const build = named("Build the app, the stand-in harness and the CLI for the window bench");
  assert.match(build, /--features e2e/);
  assert.match(build, /-p fake-harness/);
  assert.match(build, /--profile dev-release/);
  assert.match(named("The results as macos-max series points"), /node tools\/budget-series\.mjs/);
});

test("only the record job writes, on a GitHub-hosted runner, on main, after the Mac's run", () => {
  const writers = Object.entries(jobs)
    .filter(([, lines]) => lines.some((l) => /^\s+contents:\s*write\b/.test(l)))
    .map(([name]) => name);
  assert.deepEqual(writers, ["budget-record"]);
  const record = code(jobs["budget-record"]).join("\n");
  assert.match(record, /^ {4}runs-on: ubuntu-24\.04$/m);
  assert.match(record, /^ {4}needs: budget-run$/m);
  assert.match(record, /^ {4}if: github\.ref == 'refs\/heads\/main' && needs\.budget-run\.result == 'success'$/m);
  assert.match(record, /benchmark-data-dir-path: macos-max/);
  assert.match(record, /name: budget run \(macos-max\)\n/);
  assert.match(record, /name: budget run \(macos-max, evidence\)/);
  assert.doesNotMatch(record, /rust-cache|actions\/cache|cache:/);
  // The Mac never holds a token that writes.
  assert.doesNotMatch(mac, /contents:\s*write|secrets\./);
});
