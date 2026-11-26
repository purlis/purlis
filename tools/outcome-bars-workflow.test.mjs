// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `outcome-bars.yml` runs `ci.yml`'s scenario steps (`&scenario-steps`) for the macOS outcome leg,
// copied because a YAML anchor does not reach another file (ADR 0127, amended 2026-11-26). These
// tests hold the copy to the original: a step there that differs from its `ci.yml` step, comments
// aside, fails, and so does a step `ci.yml` gains that this leg would run and the copy lacks. The
// only other steps are its own for a self-hosted runner, as `release.yml`'s build has them (#7641).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

/** The steps of the list that starts after `header` (a line ending in `steps:`), each as its lines. */
export function stepsAfter(yml, header) {
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

/** A step's `if:`, or "" when it has none. A block scalar (`>-`, `|`) is refused: write it on one line. */
function conditionOf(step) {
  const condition = /^(?: {6}- | {8})if: (.*)$/m.exec(step)?.[1] ?? "";
  assert.doesNotMatch(condition, /^[>|]/, `a scenario step's if: on one line, for this reader: ${step.split("\n")[0]}`);
  return condition;
}

/**
 * Whether the macOS outcome leg (`os: macos-latest`, `outcome: true`, every other leg's flag
 * false) can run a step: its `if:` asks for neither Linux nor another leg's flag.
 */
export function runsOnTheMacOutcomeLeg(step) {
  const condition = conditionOf(step);
  return (
    !/runner\.os == 'Linux'|runner\.os != 'macOS'/.test(condition) &&
    !/(?<!!)matrix\.(sandbox|pool|specs|extras)\b/.test(condition) &&
    !/!matrix\.outcome\b|matrix\.outcome == false/.test(condition)
  );
}

const scenarioSteps = () => stepsAfter(read(".github/workflows/ci.yml"), /^ {4}steps: &scenario-steps$/);
const allBarsSteps = () => stepsAfter(read(".github/workflows/outcome-bars.yml"), /^ {4}steps:$/);
/** A step of outcome-bars.yml's own, for a self-hosted runner (`OUTCOME_BARS_RUNNER`), in no ci.yml leg. */
const selfHostedOnly = (step) => /runner\.environment == 'self-hosted'/.test(conditionOf(step));
const barsSteps = () => allBarsSteps().filter((step) => !selfHostedOnly(step));
const nameOf = (step) => /^ {6}- name: (.*)$/m.exec(step)?.[1] ?? /^ {6}- uses: (\S+)/m.exec(step)?.[1];

test("every step of outcome-bars.yml is one of ci.yml's scenario steps, word for word", () => {
  const original = new Set(scenarioSteps());
  const differ = barsSteps().filter((step) => !original.has(step));
  assert.deepEqual(differ, [], "change ci.yml's scenario step and its copy in outcome-bars.yml together");
});

test("outcome-bars.yml has every scenario step the macOS outcome leg runs, in ci.yml's order", () => {
  const want = scenarioSteps().filter(runsOnTheMacOutcomeLeg);
  assert.ok(want.length > 10, `only ${want.length} steps read from ci.yml`);
  assert.deepEqual(barsSteps(), want);
});

test("the reader takes a Linux step or another leg's step off the macOS outcome leg", () => {
  const step = (condition) => `      - name: a step\n        if: ${condition}\n        run: true`;
  assert.equal(runsOnTheMacOutcomeLeg(step("runner.os == 'Linux' && matrix.outcome")), false);
  assert.equal(runsOnTheMacOutcomeLeg(step("runner.os == 'macOS' && matrix.extras")), false);
  assert.equal(runsOnTheMacOutcomeLeg(step("matrix.pool")), false);
  assert.equal(runsOnTheMacOutcomeLeg(step("${{ !matrix.outcome }}")), false);
  assert.equal(runsOnTheMacOutcomeLeg(step("runner.os != 'macOS'")), false);
  assert.throws(() => runsOnTheMacOutcomeLeg(step(">-")), /on one line/);
  assert.equal(runsOnTheMacOutcomeLeg(step("${{ !matrix.sandbox }}")), true);
  assert.equal(runsOnTheMacOutcomeLeg(step("${{ !cancelled() && runner.os == 'macOS' && matrix.outcome }}")), true);
  assert.equal(runsOnTheMacOutcomeLeg("      - uses: ./.github/actions/rust-toolchain"), true);
});

test("ci.yml runs no macOS outcome bars of its own: outcome-bars.yml does", () => {
  const ci = read(".github/workflows/ci.yml");
  assert.doesNotMatch(ci, /^ {2}outcome-bars:$/m);
  assert.doesNotMatch(ci, /^ {4}name: outcome bars \(macos-latest\)$/m);
  const bars = read(".github/workflows/outcome-bars.yml");
  assert.match(bars, /^ {2}outcome-bars:\n {4}name: outcome bars \(macos-latest\)\n {4}# [^\n]*\n {4}# [^\n]*\n {4}continue-on-error: true$/m);
  // The triggers it had in ci.yml: main's push and the nightly, never a pull request or the queue.
  assert.match(bars, /\non:\n(?: {2}#[^\n]*\n)* {2}push:\n {4}branches: \[main\]\n {2}schedule:\n {4}- cron: "41 2 \* \* \*"\n\n/);
  assert.match(ci, /\n {2}schedule:\n {4}- cron: "41 2 \* \* \*"\n/);
});

test("outcome-bars.yml's own steps are for a self-hosted runner alone, where a persistent Mac needs them", () => {
  const steps = allBarsSteps();
  const names = steps.map(nameOf);
  const own = steps.filter(selfHostedOnly).map(nameOf);
  assert.deepEqual(own, [
    "Say that these bars ran on a self-hosted runner",
    "A self-hosted runner's workspace starts empty",
    "A self-hosted runner's Cargo, rustup and temporary folder are this job's own",
    "A self-hosted runner keeps no job folder",
  ]);
  // Emptied before the checkout; its own homes before the toolchain and rust-cache read them;
  // cleaned up last, whatever came before.
  assert.ok(names.indexOf(own[1]) < names.indexOf("actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"));
  assert.equal(names.indexOf(own[2]), names.indexOf("./.github/actions/rust-toolchain") - 1);
  assert.equal(names.at(-1), own[3]);
  assert.match(conditionOf(steps.at(-1)), /^always\(\) && runner\.environment == 'self-hosted'$/);
  assert.match(steps.at(-1), /rm -rf "\$RUNNER_TEMP\/job-home"/);
  assert.match(steps.at(-1), /sccache --stop-server/);
  // The variable picks the runner, and unset it is GitHub's.
  assert.match(read(".github/workflows/outcome-bars.yml"), /\n {4}runs-on: \$\{\{ fromJSON\(vars\.OUTCOME_BARS_RUNNER \|\| '"macos-latest"'\) \}\}\n/);
});

test("no step outcome-bars.yml runs on a self-hosted Mac takes sudo", () => {
  const sudo = allBarsSteps().filter((step) => /\bsudo\b/.test(step.replace(/^\s*#.*$/gm, "")));
  assert.ok(sudo.length > 0, "the Spotlight step was read");
  for (const step of sudo) assert.match(conditionOf(step), /runner\.environment == 'github-hosted'/, nameOf(step));
});
