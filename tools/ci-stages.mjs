// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **A pull request's run is a subset of the queue's run** (#7335, ADR 0127, spec #7324): no job
// can run on a pull request and be unable to run in the merge queue. The ready gate
// (`tools/ci-scope.mjs`) decides how much of that subset a draft and a ready pull request get;
// this file holds the workflows to the rule, so a job that the queue lacks is found when it is
// written and not when a green pull request is dropped from the queue, or never checked there.
//
// Two things are read from each workflow, line by line as the other tools read workflows:
//
// - the events it triggers on. A workflow that triggers on `pull_request` and not on
//   `merge_group` has no run in the queue at all: it must be in `PR_ONLY`, with its reason;
// - each job's `if:`, read as a pull request's run reads it and as a merge group's does
//   (`evaluate`, the same reader `ci-push-jobs.mjs` uses). Only an `if:` that is known false takes
//   a job off a run, and a job without a status function (`always()`, `!cancelled()`) is skipped
//   when a job it needs is.
//
// `ci-stages.test.mjs` runs this on the repository's workflows.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { MERGE_GROUP, PULL_REQUEST, evaluate, jobsOf } from "./ci-push-jobs.mjs";

/**
 * The workflows that run on a pull request and not in the queue, each with why. Each is a check of
 * one area, started by a path filter, and none of them is a job of the `ci` run the queue makes:
 * a pull request that touches the area gets it, and the queue never repeats it. That is a gap in
 * the rule that predates it, listed so a new one is a choice. A listing the workflow outgrows
 * (it gains a `merge_group` trigger, or is deleted) is refused.
 */
export const PR_ONLY = {
  "artifact-isolation.yml": "path-filtered probe of the artifact isolation; also runs by hand",
  "codeql.yml": "code scanning on the pull request's branch; it also runs weekly and on main",
  "docker-pool-hosts.yml": "path-filtered check of the Docker pool hosts; also runs nightly",
  "image-leftovers.yml": "path-filtered check of the image clean-up tool; also runs nightly",
  "one-login-e2e.yml": "path-filtered end-to-end check of the one login; also runs by hand",
  "one-login-proxy-listener.yml": "path-filtered check of the proxy listener; also runs by hand",
  "one-login-proxy-spike.yml": "path-filtered spike of the proxy; also runs by hand",
  "operator-image.yml": "path-filtered build of the operator image; also called by other workflows",
  "operator.yml": "path-filtered check of the operator workspace; also runs nightly and on main",
  "runner-image.yml": "path-filtered build of the runner image; also called by other workflows",
  "skill-grades.yml": "path-filtered grading of the skills; also runs nightly",
  "stories-baselines.yml": "starts on a label, to commit the baselines a pull request's stories drew",
  "stress.yml": "path-filtered stress run; also runs nightly and on main",
};

/** The events a workflow triggers on, from its `on:` block. */
export function triggersOf(text) {
  const lines = text.split("\n");
  const start = lines.findIndex((line) => /^on:/.test(line));
  if (start < 0) return new Set();
  const inline = /^on:\s*(.+)$/.exec(lines[start])?.[1]?.trim();
  if (inline) {
    return new Set(inline.replace(/^\[|\]$/g, "").split(",").map((e) => e.trim()).filter(Boolean));
  }
  const events = new Set();
  for (const line of lines.slice(start + 1)) {
    if (/^\S/.test(line) && !/^#/.test(line)) break;
    const found = /^ {2}([a-z_]+):/.exec(line);
    if (found) events.add(found[1]);
  }
  return events;
}

const STATUS = /\b(always|cancelled|failure|success)\(\)/;

/**
 * Whether job `id` of the workflow `text` can run on the event whose facts are `facts`: its `if:`
 * is not known false, and, with no status function in it, no job it needs is known unable to run.
 */
export function canRunOn(text, id, facts, jobs = jobsOf(text), seen = new Set()) {
  const job = jobs.find((one) => one.id === id);
  if (!job) throw new Error(`no job ${id} in this workflow`);
  if (job.if !== undefined && evaluate(job.if, facts) === false) return false;
  if (job.if !== undefined && STATUS.test(job.if)) return true;
  if (seen.has(id)) return true;
  seen.add(id);
  return job.needs.every((need) => canRunOn(text, need, facts, jobs, seen));
}

/** The jobs of `text` that can run on a pull request and cannot on a merge group. */
export function strandedOnQueue(text) {
  const jobs = jobsOf(text);
  return jobs
    .filter((job) => canRunOn(text, job.id, PULL_REQUEST, jobs) && !canRunOn(text, job.id, MERGE_GROUP, jobs))
    .map((job) => job.id);
}

/**
 * What breaks the rule across `workflows` (`{ file name: text }`), one line each: a workflow on a
 * pull request and not the queue that `prOnly` (`{ file name: reason }`) does not list, a listing
 * that does nothing, and each job a pull request can run and the queue cannot.
 */
export function workflowProblems(workflows, prOnly = PR_ONLY) {
  const problems = [];
  for (const [name, text] of Object.entries(workflows)) {
    const events = triggersOf(text);
    if (!events.has("pull_request")) continue;
    if (!events.has("merge_group")) {
      if (!(name in prOnly)) {
        problems.push(
          `${name} runs on a pull request and has no merge_group trigger, so the queue never runs it. ` +
            "Add the trigger, or list it in PR_ONLY in tools/ci-stages.mjs with the reason it may be a pull request's alone (#7335).",
        );
      }
      continue;
    }
    for (const id of strandedOnQueue(text)) {
      problems.push(
        `${name}: job "${id}" can run on a pull request and cannot in the merge queue. ` +
          "A pull request's run is a subset of the queue's: give it a condition the queue meets (#7335, ADR 0127).",
      );
    }
  }
  for (const [name, reason] of Object.entries(prOnly)) {
    const text = workflows[name];
    const events = text === undefined ? undefined : triggersOf(text);
    if (events === undefined || !events.has("pull_request") || events.has("merge_group")) {
      problems.push(`${name} is in PR_ONLY but is not a pull request's alone now (gone, or it runs in the queue): take it off.`);
    } else if (!reason || reason.trim() === "") {
      problems.push(`${name} is in PR_ONLY with no reason: say why a pull request may run it and the queue not.`);
    }
  }
  return problems;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const dir = process.argv[2] ?? fileURLToPath(new URL("../.github/workflows", import.meta.url));
  const workflows = Object.fromEntries(
    readdirSync(dir)
      .filter((name) => name.endsWith(".yml"))
      .map((name) => [name, readFileSync(join(dir, name), "utf8")]),
  );
  const found = workflowProblems(workflows);
  for (const line of found) console.log(line);
  if (found.length > 0) process.exit(1);
  console.log("a pull request's run is a subset of the queue's run");
}
