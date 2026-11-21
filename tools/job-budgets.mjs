// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The CI jobs' p50 budgets (spec #7324, ADR 0127, in line with ADR 0086: every budget names the
// job that measures it). `ci/job-budgets.toml` holds one budget in minutes for each job of
// `ci.yml`, by the job's id. A test refuses a job without one, so a new job brings its budget;
// `tools/ci-health.mjs` reads each job's p50 of the last seven days against it every day and files
// a ticket when a job runs more than `OVER` above it. Raising a budget takes a pull request that
// says why, so a slow job is a decision and never a drift.
//
//   node tools/job-budgets.mjs     refuses a job without a budget (the same check the tests run)
//
// The workflow and the budgets are read by the readers of the one shape each has, as
// `tools/ci-push-jobs.mjs` reads `ci.yml`: a line that is neither is refused, never skipped.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { percentile } from "./ci-health.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** The checked-in budgets, and the workflow they are for. */
export const BUDGETS_FILE = join(ROOT, "ci", "job-budgets.toml");
export const CI_FILE = join(ROOT, ".github", "workflows", "ci.yml");

/** A job's p50 may run this far above its budget before ci-health files a ticket: 20%. */
export const OVER = 0.2;

/** A job is judged once it has this many passed runs in the week: fewer say nothing yet. */
export const MIN_RUNS = 5;

/** The jobs of a workflow, `{ id, name }` each: `name` is the check run's name, a template for a matrix. */
export function ciJobs(yml) {
  const jobs = [];
  let inJobs = false;
  for (const line of yml.split("\n")) {
    if (/^\S/.test(line)) inJobs = /^jobs:\s*$/.test(line);
    if (!inJobs) continue;
    const id = /^ {2}([A-Za-z0-9_-]+):\s*$/.exec(line)?.[1];
    if (id) {
      jobs.push({ id, name: id });
      continue;
    }
    const name = /^ {4}name:\s*(.+?)\s*$/.exec(line)?.[1];
    if (name && jobs.length > 0 && jobs.at(-1).name === jobs.at(-1).id) {
      jobs.at(-1).name = name.replace(/^(["'])(.*)\1$/, "$2");
    }
  }
  return jobs;
}

const EXPRESSION = /\$\{\{.*?\}\}/g;

/**
 * The job a check run called `checkName` is of: the job with the longest literal name that fits it.
 * A matrix job whose `name:` holds no expression is run under that name with its matrix values
 * after it in parentheses (`outcome bars (macos-latest) (macos-latest, bars, 1)`), which fits too,
 * below an exact name.
 */
export function jobIdOf(checkName, jobs) {
  let best;
  let bestScore = -1;
  for (const job of jobs) {
    const literal = job.name.split(EXPRESSION);
    const source = literal.map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join(".*");
    const exact = new RegExp(`^${source}$`).test(checkName);
    if (!exact && !(literal.length === 1 && new RegExp(`^${source} \\(.*\\)$`).test(checkName))) continue;
    const score = literal.length === 1 && exact ? Infinity : literal.join("").length;
    if (score > bestScore) {
      best = job;
      bestScore = score;
    }
  }
  return best?.id;
}

/** The budgets of a file: `[[job]]` tables of an `id` and a `p50` in minutes. Throws on any other line. */
export function parseBudgets(text) {
  const budgets = [];
  for (const [i, raw] of text.split("\n").entries()) {
    const line = raw.replace(/^\s*#.*$/, "").trim();
    if (line === "") continue;
    if (line === "[[job]]") {
      budgets.push({});
      continue;
    }
    const pair = /^(?:(id)\s*=\s*"([^"]*)"|(p50)\s*=\s*(\d+(?:\.\d+)?))\s*(?:#.*)?$/.exec(line);
    if (!pair || budgets.length === 0) throw new Error(`ci/job-budgets.toml line ${i + 1}: cannot read "${raw.trim()}"`);
    const key = pair[1] ?? pair[3];
    if (key in budgets.at(-1)) throw new Error(`ci/job-budgets.toml line ${i + 1}: ${key} twice in one budget`);
    budgets.at(-1)[key] = pair[1] ? pair[2] : Number(pair[4]);
  }
  return budgets;
}

/** What is wrong with `budgets` against `jobs`, one sentence each. */
export function budgetProblems(jobs, budgets) {
  const problems = [];
  const seen = new Set();
  const known = new Set(jobs.map((job) => job.id));
  for (const [n, budget] of budgets.entries()) {
    const name = typeof budget.id === "string" && budget.id !== "" ? `"${budget.id}"` : `budget ${n + 1}`;
    if (typeof budget.id !== "string" || budget.id === "") problems.push(`${name} has no job id`);
    else if (seen.has(budget.id)) problems.push(`${name} is listed twice`);
    else {
      seen.add(budget.id);
      if (!known.has(budget.id)) problems.push(`budget for "${budget.id}": no such job in ci.yml`);
    }
    if (!(budget.p50 > 0)) problems.push(`${name} has no p50: write its budget in minutes`);
  }
  for (const job of jobs) {
    if (!seen.has(job.id)) {
      problems.push(`job "${job.id}" has no budget: add a [[job]] to ci/job-budgets.toml with its p50 in minutes`);
    }
  }
  return problems;
}

/**
 * The jobs whose p50 over `suites` (GraphQL `CheckSuite` nodes of the week's runs) is more than
 * `OVER` above their budget: `{ id, p50, budget, over, n }` (minutes), the furthest over first.
 * A job's p50 pools its matrix legs and counts only the runs it passed (a failed job ended early),
 * and a job with fewer than `MIN_RUNS` of them is not judged.
 */
export function overBudget(suites, budgets, jobs) {
  const minutes = new Map();
  for (const job of suites.flatMap((suite) => suite?.checkRuns?.nodes ?? [])) {
    if (job.conclusion !== "SUCCESS" || !job.startedAt || !job.completedAt) continue;
    const id = jobIdOf(job.name, jobs);
    if (!id) continue;
    if (!minutes.has(id)) minutes.set(id, []);
    minutes.get(id).push((Date.parse(job.completedAt) - Date.parse(job.startedAt)) / 60_000);
  }
  const over = [];
  for (const budget of budgets) {
    const seen = minutes.get(budget.id) ?? [];
    if (seen.length < MIN_RUNS) continue;
    const p50 = percentile(seen, 0.5);
    if (p50 > budget.p50 * (1 + OVER)) over.push({ id: budget.id, p50, budget: budget.p50, over: p50 / budget.p50 - 1, n: seen.length });
  }
  return over.sort((a, b) => b.over - a.over || a.id.localeCompare(b.id));
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    const problems = budgetProblems(ciJobs(readFileSync(CI_FILE, "utf8")), parseBudgets(readFileSync(BUDGETS_FILE, "utf8")));
    for (const problem of problems) console.error(`::error file=ci/job-budgets.toml::${problem}`);
    if (problems.length > 0) process.exitCode = 1;
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
