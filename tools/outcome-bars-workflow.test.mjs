// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `outcome-bars.yml` runs `ci.yml`'s scenario steps (`&scenario-steps`) for the macOS outcome leg,
// copied because a YAML anchor does not reach another file (ADR 0127, amended 2026-11-26). These
// tests hold the copy to the original: a step there that differs from its `ci.yml` step, comments
// aside, fails, and so does a step `ci.yml` gains that this leg would run and the copy lacks.
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

/** A step's `if:`, or "" when it has none. */
const conditionOf = (step) => /^(?: {6}- | {8})if: (.*)$/m.exec(step)?.[1] ?? "";

/**
 * Whether the macOS outcome leg (`os: macos-latest`, `outcome: true`, every other leg's flag
 * false) can run a step: its `if:` asks for neither Linux nor another leg's flag.
 */
export function runsOnTheMacOutcomeLeg(step) {
  const condition = conditionOf(step);
  return !/runner\.os == 'Linux'/.test(condition) && !/(?<!!)matrix\.(sandbox|pool|specs|extras)\b/.test(condition);
}

const scenarioSteps = () => stepsAfter(read(".github/workflows/ci.yml"), /^ {4}steps: &scenario-steps$/);
const barsSteps = () => stepsAfter(read(".github/workflows/outcome-bars.yml"), /^ {4}steps:$/);

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
