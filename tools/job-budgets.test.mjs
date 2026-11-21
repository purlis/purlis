// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { BUDGETS_FILE, budgetProblems, ciJobs, jobIdOf, overBudget, parseBudgets } from "./job-budgets.mjs";

const CI = new URL("../.github/workflows/ci.yml", import.meta.url);

const YML = `name: ci
jobs:
  # a comment
  rust:
    name: rust tests (part \${{ matrix.part }} of 4)
    runs-on: x
  web:
    name: web (part \${{ matrix.part }} of 4)
  scenario:
    name: scenario tests (\${{ matrix.os }}, \${{ matrix.specs && 'part' || 'runs' }})
  scenario-checks:
    name: scenario tests (macos-latest)
  commented:
    name: a job # with a comment
  nameless:
    runs-on: x
`;

test("a workflow's jobs are read with the names their check runs carry", () => {
  assert.deepEqual(ciJobs(YML), [
    { id: "rust", name: "rust tests (part ${{ matrix.part }} of 4)" },
    { id: "web", name: "web (part ${{ matrix.part }} of 4)" },
    { id: "scenario", name: "scenario tests (${{ matrix.os }}, ${{ matrix.specs && 'part' || 'runs' }})" },
    { id: "scenario-checks", name: "scenario tests (macos-latest)" },
    { id: "commented", name: "a job" },
    { id: "nameless", name: "nameless" },
  ]);
});

test("a check run belongs to the job whose name it fits, the most exact first", () => {
  const jobs = ciJobs(YML);
  assert.equal(jobIdOf("rust tests (part 3 of 4)", jobs), "rust");
  assert.equal(jobIdOf("scenario tests (macos-latest)", jobs), "scenario-checks");
  assert.equal(jobIdOf("scenario tests (ubuntu-24.04, part 2 of 6)", jobs), "scenario");
  assert.equal(jobIdOf("something no job names", jobs), undefined);
  // a matrix job named without an expression gets its matrix values after its name
  assert.equal(jobIdOf("scenario tests (macos-latest) (macos-latest, bars, 1)", jobs), "scenario-checks");
});

const BUDGETS = `# a comment
[[job]]
id = "rust"
p50 = 30

[[job]]
id = "web"
p50 = 12.5
`;

test("a budget is a job's id and its p50 in minutes", () => {
  assert.deepEqual(parseBudgets(BUDGETS), [
    { id: "rust", p50: 30 },
    { id: "web", p50: 12.5 },
  ]);
});

test("a job without a budget is refused, and so is a budget for no job", () => {
  const jobs = ciJobs(YML).slice(0, 2);
  assert.deepEqual(budgetProblems(jobs, parseBudgets(BUDGETS)), []);
  assert.match(budgetProblems(ciJobs(YML), parseBudgets(BUDGETS)).join("\n"), /job "scenario" has no budget/);
  assert.match(budgetProblems(jobs, parseBudgets(`${BUDGETS}[[job]]\nid = "gone"\np50 = 5\n`)).join("\n"), /budget for "gone": no such job/);
});

test("a budget that is no positive number, or is listed twice, is refused", () => {
  const jobs = ciJobs(YML).slice(0, 1);
  assert.match(budgetProblems(jobs, parseBudgets('[[job]]\nid = "rust"\n')).join("\n"), /"rust" has no p50/);
  assert.match(budgetProblems(jobs, parseBudgets('[[job]]\nid = "rust"\np50 = 0\n')).join("\n"), /"rust" has no p50/);
  assert.match(budgetProblems(jobs, parseBudgets(BUDGETS.replace("web", "rust"))).join("\n"), /"rust" is listed twice/);
});

test("every job in the ci workflow has a budget", () => {
  const jobs = ciJobs(readFileSync(CI, "utf8"));
  assert.ok(jobs.length > 20, "the jobs were read");
  assert.deepEqual(budgetProblems(jobs, parseBudgets(readFileSync(BUDGETS_FILE, "utf8"))), []);
});

const ran = (name, minutes, conclusion = "SUCCESS") => ({
  name,
  conclusion,
  startedAt: "2026-11-20T10:00:00Z",
  completedAt: new Date(Date.parse("2026-11-20T10:00:00Z") + minutes * 60_000).toISOString(),
});
const suitesOf = (...jobs) => jobs.map((job) => ({ checkRuns: { nodes: [job] } }));
const withRuns = (name, minutes, times) => suitesOf(...Array.from({ length: times }, () => ran(name, minutes)));

test("a job whose 7-day p50 is 25% over its budget is over, and one 15% over is not", () => {
  const jobs = ciJobs(YML);
  const budgets = parseBudgets(BUDGETS);
  const over = overBudget([...withRuns("rust tests (part 1 of 4)", 37.5, 6), ...withRuns("web (part 1 of 4)", 14.375, 6)], budgets, jobs);
  assert.deepEqual(over, [{ id: "rust", p50: 37.5, budget: 30, over: 0.25, n: 6 }]);
});

test("a job's p50 is read over all its matrix legs, and only of the runs it passed", () => {
  const jobs = ciJobs(YML);
  const budgets = parseBudgets(BUDGETS);
  const suites = [
    ...withRuns("rust tests (part 1 of 4)", 40, 3),
    ...withRuns("rust tests (part 2 of 4)", 40, 3),
    ...suitesOf(ran("rust tests (part 3 of 4)", 1, "FAILURE"), ran("rust tests (part 3 of 4)", 1, "FAILURE"), ran("rust tests (part 3 of 4)", 1, "FAILURE")),
  ];
  assert.equal(overBudget(suites, budgets, jobs)[0].p50, 40);
});

test("a job seen in too few runs says nothing yet", () => {
  assert.deepEqual(overBudget(withRuns("rust tests (part 1 of 4)", 90, 2), parseBudgets(BUDGETS), ciJobs(YML)), []);
});
