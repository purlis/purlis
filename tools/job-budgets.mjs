// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The CI jobs' p50 budgets (spec #7324, ADR 0127, in line with ADR 0086: every budget names the
// job that measures it). `ci/job-budgets.toml` holds one budget in minutes for each job of
// `ci.yml`, by the job's id; a budget with `workflow = "release"` is for a check run of
// `release.yml` instead (#7384), named in full and kept apart by the Cargo profile its runs built
// with. A test refuses a ci job without one, so a new job brings its budget;
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

import { percentile } from "./percentile.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** The checked-in budgets, and the workflow they are for. */
export const BUDGETS_FILE = join(ROOT, "ci", "job-budgets.toml");
export const CI_FILE = join(ROOT, ".github", "workflows", "ci.yml");

/**
 * The workflows besides `ci.yml` whose jobs are budgeted by id, as ci's are, each budget with
 * `workflow = "<name>"`: the macOS outcome bars, which left `ci.yml` on 2026-11-26 (ADR 0127,
 * amended that day) so that the run a dev build waits for ends when its gating jobs do.
 */
export const JOB_WORKFLOWS = { "outcome-bars": join(ROOT, ".github", "workflows", "outcome-bars.yml") };

/** The jobs of each of `JOB_WORKFLOWS`, read from the checkout: `{ [workflow]: jobs }`. */
export function workflowJobs(read = (path) => readFileSync(path, "utf8")) {
  return Object.fromEntries(Object.entries(JOB_WORKFLOWS).map(([name, path]) => [name, ciJobs(read(path))]));
}
export const RELEASE_PROFILES = ["dev-release", "release"];

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
    const name = /^ {4}name:\s*(.+?)\s*(?:\s#.*)?$/.exec(line)?.[1];
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

/**
 * The budgets of a file: `[[job]]` tables of an `id` and a `p50` in minutes. A release budget adds
 * `workflow = "release"`, the `profile` its runs build with, the `check` run's name and the `run` it
 * was measured from. Throws on any other line.
 */
export function parseBudgets(text) {
  const budgets = [];
  for (const [i, raw] of text.split("\n").entries()) {
    const line = raw.replace(/^\s*#.*$/, "").trim();
    if (line === "") continue;
    if (line === "[[job]]") {
      budgets.push({});
      continue;
    }
    const pair = /^(?:(id|workflow|profile|check)\s*=\s*"([^"]*)"|(p50|run)\s*=\s*(\d+(?:\.\d+)?))\s*(?:#.*)?$/.exec(line);
    if (!pair || budgets.length === 0) throw new Error(`ci/job-budgets.toml line ${i + 1}: cannot read "${raw.trim()}"`);
    const key = pair[1] ?? pair[3];
    if (key in budgets.at(-1)) throw new Error(`ci/job-budgets.toml line ${i + 1}: ${key} twice in one budget`);
    budgets.at(-1)[key] = pair[1] ? pair[2] : Number(pair[4]);
  }
  return budgets;
}

/**
 * What is wrong with `budgets` against `jobs` (ci's) and `others` (`{ [workflow]: jobs }`, the jobs
 * of `JOB_WORKFLOWS`), one sentence each.
 */
export function budgetProblems(jobs, budgets, others = {}) {
  const problems = [];
  const seen = new Set();
  const known = new Set(jobs.map((job) => job.id));
  for (const [n, budget] of budgets.entries()) {
    const name = typeof budget.id === "string" && budget.id !== "" ? `"${budget.id}"` : `budget ${n + 1}`;
    if (typeof budget.id !== "string" || budget.id === "") problems.push(`${name} has no job id`);
    else if (seen.has(budget.id)) problems.push(`${name} is listed twice`);
    else if (budget.workflow in others) {
      seen.add(budget.id);
      if (!others[budget.workflow].some((job) => job.id === budget.id)) {
        problems.push(`budget for "${budget.id}": no such job in ${budget.workflow}.yml`);
      }
    } else {
      seen.add(budget.id);
      if (budget.workflow === "release") {
        if (!RELEASE_PROFILES.includes(budget.profile)) problems.push(`${name} has no profile: ${RELEASE_PROFILES.join(" or ")}`);
        if (!budget.check) problems.push(`${name} names no check run`);
        if (!(budget.run > 0)) problems.push(`${name} names no run it was measured from (ADR 0086)`);
      } else if (budget.workflow !== undefined && budget.workflow !== "ci") problems.push(`${name} is for workflow "${budget.workflow}", which is not ci, release or one of ${Object.keys(others).join(", ") || "no other"}`);
      else if (!known.has(budget.id)) problems.push(`budget for "${budget.id}": no such job in ci.yml`);
    }
    if (!(budget.p50 > 0)) problems.push(`${name} has no p50: write its budget in minutes`);
  }
  const checks = new Set();
  for (const budget of budgets.filter((b) => b.workflow === "release" && b.check && b.profile)) {
    const key = `${budget.profile}: ${budget.check}`;
    if (checks.has(key)) problems.push(`"${budget.id}" budgets ${key} a second time`);
    checks.add(key);
  }
  for (const job of jobs) {
    if (!seen.has(job.id)) {
      problems.push(`job "${job.id}" has no budget: add a [[job]] to ci/job-budgets.toml with its p50 in minutes`);
    }
  }
  for (const [workflow, list] of Object.entries(others)) {
    for (const job of list) {
      const budget = budgets.find((b) => b.id === job.id);
      if (!budget) {
        problems.push(`job "${job.id}" of ${workflow}.yml has no budget: add a [[job]] with workflow = "${workflow}" to ci/job-budgets.toml`);
      } else if (budget.workflow !== workflow) {
        problems.push(`job "${job.id}" of ${workflow}.yml is budgeted without workflow = "${workflow}"`);
      }
    }
  }
  return problems;
}

/**
 * The jobs whose p50 over `suites` (GraphQL `CheckSuite` nodes of the week's runs) is more than
 * `OVER` above their budget: `{ id, p50, budget, over, n }` (minutes), the furthest over first.
 * A job's p50 pools its matrix legs and counts only the runs it passed (a failed job ended early),
 * and a job with fewer than `MIN_RUNS` of them is not judged. `workflow` is the workflow `suites`
 * and `jobs` are of: `ci`, or one of `JOB_WORKFLOWS`, whose runs are read apart.
 */
export function overBudget(suites, budgets, jobs, workflow = "ci") {
  budgets = budgets.filter((budget) => (budget.workflow ?? "ci") === workflow);
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
    judge(over, budget, seen);
  }
  return worstFirst(over);
}

const worstFirst = (over) => over.sort((a, b) => b.over - a.over || a.id.localeCompare(b.id));

function judge(over, budget, seen) {
  const p50 = percentile(seen, 0.5);
  if (p50 <= budget.p50 * (1 + OVER)) return;
  over.push({
    id: budget.id,
    ...(budget.workflow ? { workflow: budget.workflow, profile: budget.profile } : {}),
    p50,
    budget: budget.p50,
    over: p50 / budget.p50 - 1,
    n: seen.length,
  });
}

/**
 * The Cargo profile a `release.yml` run built with, read from its title (`run-name`): a dev build of
 * main is `dev-release`, the nightly profile guard and a stable tag are `release`. A build started
 * by hand may ask for either, so it is neither and no budget judges it.
 */
export function releaseProfile(title) {
  if (/^dev build of /.test(title ?? "")) return "dev-release";
  if (title === "profile guard" || /^stable /.test(title ?? "")) return "release";
  return undefined;
}

/**
 * `overBudget` for `release.yml`: the release budgets whose p50 over the week's `runs` (REST
 * workflow runs, with their `display_title`) and `suites` (their check suites, matched by
 * `workflowRun.databaseId`) is more than `OVER` above. A job is read in the runs of its budget's
 * profile only, by its check run's exact name, so a dev build never counts against the `release`
 * profile's budget.
 */
export function overReleaseBudget(runs, suites, budgets) {
  const profileOf = new Map(runs.map((run) => [run.id, releaseProfile(run.display_title)]));
  const over = [];
  for (const budget of budgets.filter((b) => b.workflow === "release")) {
    const seen = [];
    for (const suite of suites) {
      if (profileOf.get(suite?.workflowRun?.databaseId) !== budget.profile) continue;
      for (const job of suite.checkRuns?.nodes ?? []) {
        if (job.name !== budget.check || job.conclusion !== "SUCCESS" || !job.startedAt || !job.completedAt) continue;
        seen.push((Date.parse(job.completedAt) - Date.parse(job.startedAt)) / 60_000);
      }
    }
    if (seen.length >= MIN_RUNS) judge(over, budget, seen);
  }
  return worstFirst(over);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    const problems = budgetProblems(
      ciJobs(readFileSync(CI_FILE, "utf8")),
      parseBudgets(readFileSync(BUDGETS_FILE, "utf8")),
      workflowJobs(),
    );
    for (const problem of problems) console.error(`::error file=ci/job-budgets.toml::${problem}`);
    if (problems.length > 0) process.exitCode = 1;
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
