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
import { pullRequestPathsOf } from "./ci-scope.mjs";

/** The shortest reason, in characters, that can say why: a label such as "path-filtered" cannot. */
export const MIN_REASON = 40;

/**
 * The workflows kept on pull requests alone **on purpose**, each with why (#7420). Every other
 * workflow a pull request starts by `paths:` runs in the queue too, on the paths of its own
 * filter (`workflow-scope`, below). A workflow is listed here only when a queue run would add
 * nothing a pull request's run, `main`'s or the nightly's does not, or has nothing to run on:
 *
 * - it reports and gates nothing (`continue-on-error`, or a dry run of a publish);
 * - the paths it watches are its own, so no other change can break it between a pull request's
 *   run and the queue's;
 * - what it needs is not a merge group's: a label, a model's paid answer.
 *
 * A listing the workflow outgrows (it gains a `merge_group` trigger, or is deleted) is refused,
 * and so is a reason too short to be one (`MIN_REASON`).
 */
export const PR_ONLY = {
  "artifact-isolation.yml":
    "an informational probe (continue-on-error) of three operating systems that gates nothing, on paths no other change touches: a queue run would hold a macOS and a Windows runner for no check",
  "codeql.yml":
    "its alerts belong to the pull request's branch and to main's own scan after the merge (it also runs weekly); a merge group's branch is not one code scanning reports on",
  "image-leftovers.yml":
    "a report of what a failed run left in the registry, kept continue-on-error so it gates nothing; the tool's own tests run in the queue's ci run, and the report runs nightly",
  "one-login-proxy-spike.yml":
    "the spike's own tests/one-login-spike tree and nothing else starts it, so no sibling can break it between a pull request's run and the queue's, and it gates no shipped code",
  "operator-image.yml":
    "a dry run of the operator image's publish, whose real run is main's, called by operator.yml after its checks pass; the queue's operator.yml run covers what the build reads",
  "parity-fixture-images.yml":
    "a dry run of the parity fixture images' publish, whose real run is main's; its paths are the fixtures' own files, and the queue's remote parity run builds any fixture a change moved past its pin",
  "runner-image.yml":
    "a dry run of the runner image's publish, whose real run is the release's call; nothing is published from a pull request, and the queue's ci run builds what the image holds",
  "skill-grades.yml":
    "it grades the skills with a paid model's answers, from an environment's keys that carry a spend limit; it runs nightly, and a queue run would spend them on every merge",
  "stories-baselines.yml":
    "it starts on a label, to commit the baselines a pull request's stories drew: a merge group has no labels and no pull request branch to commit to",
  "stress.yml":
    "the one job a pull request runs is a timing measurement at 300,000 files that a shared queue runner would make flaky; main's push and the nightly record the rest",
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

/** The workflow a file's `workflow-scope` step names, or `undefined` when it has none. */
const scopedBy = (text) =>
  /uses: \.\/\.github\/actions\/workflow-scope\s*\n\s*with:\s*\n\s*workflow: (\S+)/.exec(text)?.[1];

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
    if (pullRequestPathsOf(text) !== null && scopedBy(text) !== name) {
      problems.push(
        `${name} starts a pull request's run by \`paths:\` and a merge group's by nothing, so every queue run would pay for it. ` +
          `Add a job that uses ./.github/actions/workflow-scope with \`workflow: ${name}\`, and give the jobs \`github.event_name != 'merge_group' || needs.scope.outputs.run != 'false'\` (#7420).`,
      );
    }
    if (scopedBy(text) !== undefined) {
      if (/^ {4}paths-ignore:|^ {4}paths:[ \t]*\S/m.test(text)) {
        problems.push(`${name}: its pull_request filter is not a block \`paths:\` list, which workflow-scope cannot read (#7420).`);
      }
      for (const one of jobsOf(text).filter((j) => j.id !== "scope" && canRunOn(text, j.id, MERGE_GROUP))) {
        if (!one.needs.includes("scope") || !/needs\.scope\.outputs\.run/.test(one.if ?? "")) {
          problems.push(`${name}: job "${one.id}" must need \`scope\` and run only when \`needs.scope.outputs.run != 'false'\` in a merge group (#7420).`);
        }
      }
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
    } else if (!reason || reason.trim().length < MIN_REASON) {
      problems.push(
        `${name} is in PR_ONLY with no reason (a sentence of at least ${MIN_REASON} characters): say why a pull request may run it and the queue not.`,
      );
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
