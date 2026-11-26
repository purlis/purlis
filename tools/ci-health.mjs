// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The daily CI health report (#3598, daily and acting from spec #7324): how the `ci` workflow did
// over the last seven days, and a ticket for each flaky test it finds. The jobs of the workflows
// `JOB_WORKFLOWS` names (`tools/job-budgets.mjs`), which left `ci.yml` but not its watch, are read
// beside its own: their durations, budgets, red jobs and flaky tests (`sideWeek`).
//
// CI got twice as slow and three times as flaky between 2026-10-14 and 10-27, and nobody saw it
// for two weeks. The `ci-health` workflow runs this every day and writes the week's numbers
// into its job summary and into one issue labelled `area:ci`, so the next slowdown shows on its
// first week: runs a day, how many went green, how many only on a rerun, the merge queue's wait,
// each job's p50 and p90, every flaky test by name, and the slowest tests.

import { createHash } from "node:crypto";
import { appendFileSync, readFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { requiredDrift } from "./ci-push-jobs.mjs";
import { requiredNames } from "./land.mjs";
import { percentile } from "./percentile.mjs";
import {
  BUDGETS_FILE,
  CI_FILE,
  JOB_WORKFLOWS,
  OVER,
  ciJobs,
  overBudget,
  overReleaseBudget,
  parseBudgets,
  workflowJobs,
} from "./job-budgets.mjs";
import { CAP, QUARANTINE_FILE, parseQuarantine } from "./quarantine.mjs";

export { percentile };

const seconds = (from, to) => Math.round((Date.parse(to) - Date.parse(from)) / 1000);
const spread = (values) => ({ n: values.length, p50: percentile(values, 0.5), p90: percentile(values, 0.9) });

/**
 * The conclusions that count as red: a failure, a run its job limit ended, and a run whose
 * workflow could not start. `cancelled` (a newer push or the merge queue superseded it),
 * `action_required` (a fork's run waiting for approval) and `skipped` are neither red nor green.
 */
export const RED = ["failure", "timed_out", "startup_failure"];

/**
 * What the runs list says of a week of `ci` runs (`GET /actions/workflows/ci.yml/runs`, which
 * answers each run's latest attempt).
 */
export function runStats(runs) {
  const perDay = {};
  const conclusions = {};
  const walls = {};
  let greenOnRerun = 0;
  for (const run of runs) {
    const day = run.created_at.slice(0, 10);
    perDay[day] = (perDay[day] ?? 0) + 1;
    const conclusion = run.status === "completed" ? run.conclusion : run.status;
    conclusions[conclusion] = (conclusions[conclusion] ?? 0) + 1;
    if (conclusion !== "success") continue;
    if (run.run_attempt > 1) greenOnRerun += 1;
    (walls[run.event] ??= []).push(seconds(run.run_started_at, run.updated_at));
  }
  const decided = (conclusions.success ?? 0) + RED.reduce((sum, red) => sum + (conclusions[red] ?? 0), 0);
  const wall = Object.fromEntries(Object.entries(walls).map(([event, values]) => [event, spread(values)]));
  return {
    runs: runs.length,
    perDay,
    conclusions,
    successRate: decided === 0 ? null : (conclusions.success ?? 0) / decided,
    greenOnRerun,
    wall,
  };
}

/**
 * Merge-queue time over the week's merged pull requests (GraphQL `search`, each with its
 * `AddedToMergeQueueEvent`s and `RemovedFromMergeQueueEvent`s): from the last time it entered the
 * queue to its merge, which is the wait an author sees. Each removal that was not the merge is
 * counted by its reason, when it fell within the week (`from`..`to`, whole UTC days) if one is
 * given: a pull request merged this week may have been put out last week. A pull request put
 * out and not merged by the week's end is not among `prs`, so its removals are not counted.
 */
export function queueStats(prs, { from, to } = {}) {
  const inWeek = (at) => (!from || at.slice(0, 10) >= from) && (!to || at.slice(0, 10) <= to);
  const waits = [];
  const ejected = {};
  for (const pr of prs) {
    let entered = null;
    for (const event of pr.timelineItems?.nodes ?? []) {
      if (event.__typename === "AddedToMergeQueueEvent") entered = event.createdAt;
      if (event.__typename === "RemovedFromMergeQueueEvent" && event.reason !== "merged" && inWeek(event.createdAt)) {
        ejected[event.reason] = (ejected[event.reason] ?? 0) + 1;
      }
    }
    if (entered !== null && pr.mergedAt) waits.push(seconds(entered, pr.mergedAt));
  }
  return { merged: prs.length, ...spread(waits), ejected };
}

/**
 * Each job's duration over the runs of `suites` (GraphQL `CheckSuite` nodes) it passed in, as
 * `{ name, n, p50, p90 }`, slowest p90 first. A job that failed, was cancelled or was skipped
 * ended early, and its time would pull the spread down.
 */
export function jobStats(suites) {
  const durations = new Map();
  for (const job of suites.flatMap((suite) => suite?.checkRuns?.nodes ?? [])) {
    if (job.conclusion !== "SUCCESS" || !job.startedAt || !job.completedAt) continue;
    if (!durations.has(job.name)) durations.set(job.name, []);
    durations.get(job.name).push(seconds(job.startedAt, job.completedAt));
  }
  return [...durations]
    .map(([name, values]) => ({ name, ...spread(values) }))
    .sort((a, b) => b.p90 - a.p90 || a.name.localeCompare(b.name));
}

/**
 * The jobs that were red in `suites` (of runs that ended green), as `{ name, times }`, the most
 * often first. An evidence-only job's miss turns neither the run red nor anything else this report
 * counts, so without this it goes quiet (#4465). Needing no list of such jobs, it names any job
 * `continue-on-error` holds. A job cancelled or skipped said nothing.
 */
export function evidenceRed(suites) {
  const counts = new Map();
  for (const job of suites.flatMap((suite) => suite?.checkRuns?.nodes ?? [])) {
    if (job.conclusion === "FAILURE" || job.conclusion === "TIMED_OUT") {
      counts.set(job.name, (counts.get(job.name) ?? 0) + 1);
    }
  }
  return [...counts]
    .map(([name, times]) => ({ name, times }))
    .sort((a, b) => b.times - a.times || a.name.localeCompare(b.name));
}

/** The warning `tools/slowest-tests.mjs` writes for a test that passed on a retry. */
const FLAKY = /^Flaky \(passed on retry \d+\): (.+)$/;

/** Every test a job of `suites` called flaky, as `{ test, count }`, the most often first. */
export function flakyTests(suites) {
  const counts = new Map();
  for (const job of suites.flatMap((suite) => suite?.checkRuns?.nodes ?? [])) {
    for (const annotation of job.annotations?.nodes ?? []) {
      const flaky = FLAKY.exec(annotation.title ?? "");
      if (flaky) counts.set(flaky[1], (counts.get(flaky[1]) ?? 0) + 1);
    }
  }
  return [...counts]
    .map(([test, count]) => ({ test, count }))
    .sort((a, b) => b.count - a.count || a.test.localeCompare(b.test));
}

/**
 * Every flaky test of `suites` with where it flaked, for its ticket: `{ test, count, jobs, runs }`,
 * the most often first. `jobs` are the suites (the job names) that ran it, `runs` the workflow runs
 * it flaked in (`{ id, url, createdAt }`, each once), read from each suite's `workflowRun`.
 */
export function flakeEvidence(suites) {
  const found = new Map();
  for (const suite of suites) {
    for (const job of suite?.checkRuns?.nodes ?? []) {
      for (const annotation of job.annotations?.nodes ?? []) {
        const flaky = FLAKY.exec(annotation.title ?? "");
        if (!flaky) continue;
        const one = found.get(flaky[1]) ?? { test: flaky[1], count: 0, jobs: [], runs: [] };
        one.count += 1;
        if (!one.jobs.includes(job.name)) one.jobs.push(job.name);
        const run = suite.workflowRun;
        if (run && !one.runs.some((seen) => seen.id === run.databaseId)) {
          one.runs.push({ id: run.databaseId, url: run.url, createdAt: run.createdAt });
        }
        found.set(flaky[1], one);
      }
    }
  }
  return [...found.values()].sort((a, b) => b.count - a.count || a.test.localeCompare(b.test));
}

const TICKET_TITLE = "Flaky test: ";
const TEST_MARKER = /<!-- flake-test: (.*?) -->/;
const RUNS_MARKER = /<!-- flake-runs: ([\d,]*) -->/;
// Runs from before a close that a reopen skipped: GitHub clears `closed_at` on a reopen, so the
// ticket itself has to remember which runs its fix already answered.
const ANSWERED_MARKER = /<!-- flake-answered: ([\d,]*) -->/;
const idsOf = (marker, body) => (marker.exec(body ?? "")?.[1] ?? "").split(",").filter(Boolean).map(Number);
// Every ">" is escaped, so no test id can end the comment it sits in (`-->` or `--!>`).
const safe = (test) => test.replace(/>/g, "&gt;");
const marked = (test, ids) => `<!-- flake-test: ${safe(test)} -->\n<!-- flake-runs: ${ids.join(",")} -->`;
const runList = (runs) => runs.map((run) => `- ${run.url}`).join("\n");

const ticketTitle = (test) => `${TICKET_TITLE}${test}`.slice(0, 200);

/** The issue of `issues` that is `test`'s flake ticket: by the test id in its body, then by its title. */
export function ticketFor(test, issues) {
  return (
    issues.find((issue) => TEST_MARKER.exec(issue.body ?? "")?.[1] === safe(test)) ??
    issues.find((issue) => issue.title === ticketTitle(test))
  );
}

/**
 * The runs of `runs` that count against `ticket`. An open ticket is owed all of them. A closed
 * ticket was closed by a fix, so a run counts only if it STARTED (was created) after the ticket's last close
 * (`closed_at`): an earlier run is history the fix already answered, and a run with no start time
 * is treated as history too, since reopening on a guess is the defect this rule exists for. Time,
 * not "the run's head contains the fixing commit": a close needs no commit to read back (a
 * ticket can be closed by hand, or by a PR that only mentions it), and a run in flight at the
 * close, on a head from before the fix, is the one case that rule would catch and this does not;
 * it is rare, and the next run on the fix's head tells the truth.
 */
export function runsAfterClose(runs, ticket) {
  if (ticket?.state !== "closed") {
    const answered = idsOf(ANSWERED_MARKER, ticket?.body);
    return runs.filter((run) => !answered.includes(run.id));
  }
  const closed = Date.parse(ticket.closed_at ?? "");
  // A closed ticket with no close time cannot be told from history: count nothing.
  if (Number.isNaN(closed)) return [];
  return runs.filter((run) => Date.parse(run.createdAt ?? "") > closed);
}

/**
 * What to do with the flakes `flakes` (`flakeEvidence`) against the `area:ci` issues `issues` (any
 * state): one new ticket for a test with none, and a comment, naming only the runs the ticket does
 * not name yet, on the ticket it has. A ticket is found by the test id in its body (the title may
 * be edited by hand), then by its title. `create` is `{ title, body, labels }`; `comment` is
 * `{ number, body, update, reopen }`, `update` being the ticket's body with the new runs added.
 */
export function planFlakeTickets(flakes, issues) {
  const create = [];
  const comment = [];
  for (const flake of flakes) {
    const title = ticketTitle(flake.test);
    const ticket = ticketFor(flake.test, issues);
    if (!ticket) {
      create.push({
        title,
        labels: [ISSUE_LABEL],
        body: [
          marked(flake.test, flake.runs.map((run) => run.id)),
          `\`${flake.test}\` passed on a retry in ${plural(flake.runs.length, "run")}, in ${flake.jobs.map((job) => `\`${job}\``).join(", ")}.`,
          "",
          "A test that fails and then passes on its one retry is a flake (ADR 0127): green, and never silent. " +
            "Three flakes in seven days put it in `ci/quarantine.toml`.",
          "",
          "Runs:",
          runList(flake.runs),
          "",
          "Filed by `ci-health`. It adds each new run of this flake as a comment here.",
        ].join("\n"),
      });
      continue;
    }
    const known = idsOf(RUNS_MARKER, ticket.body);
    const counted = runsAfterClose(flake.runs, ticket);
    const fresh = counted.filter((run) => !known.includes(run.id));
    if (fresh.length === 0) continue;
    const ids = [...known, ...fresh.map((run) => run.id)];
    const answered = [...new Set([...idsOf(ANSWERED_MARKER, ticket.body), ...flake.runs.filter((run) => !counted.includes(run)).map((run) => run.id)])];
    const withAnswered = (text) =>
      answered.length === 0
        ? text
        : ANSWERED_MARKER.test(text)
          ? text.replace(ANSWERED_MARKER, `<!-- flake-answered: ${answered.join(",")} -->`)
          : `<!-- flake-answered: ${answered.join(",")} -->\n${text}`;
    const body = ticket.body ?? "";
    comment.push({
      number: ticket.number,
      reopen: ticket.state === "closed",
      body: `Flaked again in ${plural(fresh.length, "run")}, in ${flake.jobs.map((job) => `\`${job}\``).join(", ")}:\n${runList(fresh)}`,
      update: withAnswered(
        RUNS_MARKER.test(body)
          ? body.replace(RUNS_MARKER, `<!-- flake-runs: ${ids.join(",")} -->`)
          : `${marked(flake.test, ids)}\n${body}`,
      ),
    });
  }
  return { create, comment };
}

/**
 * Ready to merged over the week's merged pull requests, as `{ n, p50, p90 }` in seconds: from the
 * last time the pull request was marked ready for review (`ReadyForReviewEvent`), or from its
 * opening when it never was a draft, to its merge. The number the spec #7324 is judged by.
 */
export function readyToMerged(prs) {
  const waits = [];
  for (const pr of prs) {
    if (!pr.mergedAt) continue;
    const readies = (pr.timelineItems?.nodes ?? [])
      .filter((event) => event.__typename === "ReadyForReviewEvent" && event.createdAt <= pr.mergedAt)
      .map((event) => event.createdAt)
      .sort();
    waits.push(seconds(readies.at(-1) ?? pr.createdAt, pr.mergedAt));
  }
  return spread(waits);
}

/**
 * The one reason GitHub gives for a queue drop, as read on 60 merged pull requests of purlis/purlis:
 * `failed_checks` (13), beside `merge_conflict` (2, which builds no queue run) and `merged` (27).
 */
export const DROP_REASON = "failed_checks";

/**
 * The queue drop rate over the week's merged pull requests: `{ entries, drops, rate }`. An entry is
 * each time a pull request entered the queue; a drop is a removal for `failed_checks`, because its
 * queue run failed. A merge conflict is not one (it builds no queue run), nor is a merge. As in
 * `queueStats`, a pull request not merged by the week's end is not among `prs`.
 */
export function queueDrops(prs, { from, to } = {}) {
  const inWeek = (at) => (!from || at.slice(0, 10) >= from) && (!to || at.slice(0, 10) <= to);
  let entries = 0;
  let drops = 0;
  for (const pr of prs) {
    for (const event of pr.timelineItems?.nodes ?? []) {
      if (!inWeek(event.createdAt)) continue;
      if (event.__typename === "AddedToMergeQueueEvent") entries += 1;
      if (event.__typename === "RemovedFromMergeQueueEvent" && event.reason === DROP_REASON) {
        drops += 1;
      }
    }
  }
  return { entries, drops, rate: entries === 0 ? null : drops / entries };
}

/** Flakes in this many runs of the week put a test in quarantine (spec #7324). */
export const QUARANTINE_AFTER = 3;

/** A quarantined test's ticket gets this label once the test has waited this many days. */
export const PRIORITY_LABEL = "priority:p1";
export const PRIORITY_AFTER_DAYS = 14;

const wholeDays = (from, to) => Math.round((Date.parse(to) - Date.parse(from)) / 86_400_000);

/**
 * The pull requests to open that add flaky tests to the quarantine: a test that flaked in
 * `QUARANTINE_AFTER` runs of the week, with its ticket filed (`issues`), not in `entries` already,
 * and with no such pull request open (`openBranches`). None once the list is at the cap (`CAP`). `{ id, ticket, added, branch, title, body }`.
 * A test flaking in as many runs as it takes is read from the `--sample` of the week's runs, so a
 * test flaking less often than that can still reach it: the proposal is a PR the steward reviews.
 */
export function planQuarantine({ flakes, entries, issues, openBranches, today }) {
  const listed = new Set(entries.map((entry) => entry.id));
  const proposals = [];
  for (const flake of flakes) {
    if (entries.length + proposals.length >= CAP) break;
    if (flake.runs.length < QUARANTINE_AFTER || listed.has(flake.test)) continue;
    const ticket = ticketFor(flake.test, issues);
    if (!ticket) continue;
    // Only the runs since the ticket's last close count: a fixed flake's old runs are answered.
    const counted = runsAfterClose(flake.runs, ticket);
    if (counted.length < QUARANTINE_AFTER) continue;
    const branch = `ci-health/quarantine-${createHash("sha256").update(flake.test).digest("hex").slice(0, 12)}`;
    if (openBranches.includes(branch)) continue;
    proposals.push({
      id: flake.test,
      ticket: ticket.number,
      added: today,
      branch,
      title: `Quarantine ${flake.test}`.slice(0, 200),
      body: [
        `\`${flake.test}\` passed on a retry in ${counted.length} runs of the last seven days (#${ticket.number}), ` +
          `which is a flake three times over (ADR 0127). Until its fix lands, only the nightly run runs it.`,
        "",
        "Runs:",
        runList(counted),
        "",
        `The pull request that fixes it removes this entry from \`ci/quarantine.toml\`. Opened by \`ci-health\`; review and enqueue it.`,
      ].join("\n"),
    });
  }
  return proposals;
}

/** `text` (the quarantine file) with one more entry at its end. */
export function appendEntry(text, { id, ticket, added }) {
  const quoted = id.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  return `${text.replace(/\n*$/, "\n")}\n[[test]]\nid = "${quoted}"\nticket = ${ticket}\nadded = "${added}"\n`;
}

/**
 * The quarantine's verdict: `red` when more than `CAP` tests are in it, so it cannot become a
 * graveyard, with the sentence that pings the operator; and `prioritise`, the ticket of each
 * entry `PRIORITY_AFTER_DAYS` old or more, for the priority label.
 */
export function quarantineVerdict(entries, { today }) {
  const red = entries.length > CAP;
  return {
    red,
    message: red
      ? `${entries.length} tests are quarantined, more than the ${CAP} the quarantine allows (ci/quarantine.toml): fix some before more are added.`
      : "",
    prioritise: entries
      .map((entry) => ({ ticket: entry.ticket, label: PRIORITY_LABEL, days: wholeDays(entry.added, today) }))
      .filter((entry) => entry.days >= PRIORITY_AFTER_DAYS),
  };
}

const BUDGET_TITLE = "Job over budget: ";
const BUDGET_MARKER = /<!-- budget-job: (.*?) -->/;

/**
 * The tickets for jobs over their budget (`overBudget`): `create` is `{ title, body, labels }`, one
 * for each job that has no open ticket of its own (found by the job id in its body, then by its
 * title). A closed ticket does not hold the job: its budget was raised or its time fixed, and it
 * has gone over again.
 */
export function planBudgetTickets(over, issues) {
  const open = issues.filter((issue) => issue.state !== "closed");
  const create = [];
  for (const job of over) {
    const title = `${BUDGET_TITLE}${job.id}`;
    const held = open.some((issue) => BUDGET_MARKER.exec(issue.body ?? "")?.[1] === job.id || issue.title === title);
    if (held) continue;
    create.push({
      title,
      labels: [ISSUE_LABEL],
      body: [
        `<!-- budget-job: ${job.id} -->`,
        `The \`${job.id}\` job of \`${job.workflow ?? "ci"}.yml\`${job.profile ? ` (a \`${job.profile}\` profile build)` : ""} took a p50 of ${job.p50.toFixed(1)} min over ${plural(job.n, "passed run")} of the last seven days: ` +
          `${Math.round(job.over * 100)}% over its budget of ${job.budget} min in \`ci/job-budgets.toml\`.`,
        "",
        `A job is filed when it runs more than ${Math.round(OVER * 100)}% over. Make the job faster, or raise its budget in a pull request that says why ` +
          "(spec #7324, ADR 0127, ADR 0086).",
        "",
        "Filed by `ci-health`.",
      ].join("\n"),
    });
  }
  return { create };
}

/** How many tests, flaky tests and jobs the report lists. */
export const LIMIT = 20;

/**
 * The slowest tests of the rust jobs whose annotations are `lists` (one list per job, as
 * `GET /check-runs/{id}/annotations` answers), read from the "Slowest Rust tests" notice
 * `tools/slowest-tests.mjs` writes: `{ test, seconds, runs }`, each test's longest time and how
 * many of the jobs named it, slowest first.
 */
export function slowestTests(lists, { limit = LIMIT } = {}) {
  const tests = new Map();
  for (const list of lists) {
    for (const annotation of list) {
      if (annotation.title !== "Slowest Rust tests") continue;
      for (const line of (annotation.message ?? "").split("\n")) {
        const entry = /^\s*([\d.]+) s {2}(.+)$/.exec(line);
        if (!entry) continue;
        const [, time, test] = entry;
        const seen = tests.get(test) ?? { test, seconds: 0, runs: 0 };
        seen.seconds = Math.max(seen.seconds, Number(time));
        seen.runs += 1;
        tests.set(test, seen);
      }
    }
  }
  return [...tests.values()].sort((a, b) => b.seconds - a.seconds).slice(0, limit);
}

/** The most runs one filtered runs list answers, however many pages are asked for. */
export const LIST_CAP = 1000;

/**
 * The `created` filters that read `day`'s `total` runs: the day itself when one list holds them
 * all, or equal slices of it small enough that each does (900 a slice leaves room for a busy hour).
 */
export function runWindows(day, total) {
  if (total <= LIST_CAP) return [day];
  const slices = Math.ceil(total / 900);
  const start = Date.parse(`${day}T00:00:00Z`);
  const stamp = (ms) => new Date(ms).toISOString().replace(".000Z", "Z");
  return Array.from({ length: slices }, (_, i) => {
    const from = start + Math.round((i * 86_400_000) / slices);
    const to = start + Math.round(((i + 1) * 86_400_000) / slices) - 1000;
    return `${stamp(from)}..${stamp(to)}`;
  });
}

/**
 * The report's issue, looked for page by page (`listPage(n)` answers page `n` of the `area:ci`
 * issues, 100 a page) until it is found or the pages run out: the label holds every CI issue,
 * and the report's may be far down the list.
 */
export async function locateIssue(listPage) {
  for (let page = 1; page <= 100; page += 1) {
    const issues = await listPage(page);
    const found = findIssue(issues);
    if (found || issues.length === 0) return found;
  }
  return undefined;
}

/** `--days N --sample N --issue`, each number a positive whole one. */
export function parseArgs(argv) {
  const number = (name, fallback) => {
    const at = argv.indexOf(name);
    if (at < 0) return fallback;
    const value = argv[at + 1] ?? "";
    if (!/^[1-9]\d*$/.test(value)) throw new Error(`${name} takes a positive whole number, not "${value}"`);
    return Number(value);
  };
  return { days: number("--days", 7), sample: number("--sample", 600), issue: argv.includes("--issue") };
}

/** At most `max` of `items`, spread evenly over them; all of them when there are no more. */
export function sample(items, max) {
  if (items.length <= max) return items;
  return Array.from({ length: max }, (_, i) => items[Math.floor((i * items.length) / max)]);
}

/** The issue the report is kept in: one, found by this title and the `area:ci` label. */
export const ISSUE_TITLE = "CI health: the daily report";
/** What the issue was called while the report was weekly: found, and renamed, never filed twice. */
export const LEGACY_ISSUE_TITLE = "CI health: the weekly report";
export const ISSUE_LABEL = "area:ci";

/** The report's issue among `issues` (`GET /issues`), or undefined when there is none yet. */
export function findIssue(issues) {
  return issues.find(
    (issue) => [ISSUE_TITLE, LEGACY_ISSUE_TITLE].includes(issue.title) && issue.labels.some((label) => (label.name ?? label) === ISSUE_LABEL),
  );
}

const minutes = (s) => (s === null ? "–" : (s / 60).toFixed(1));
const percent = (rate) => (rate === null ? "–" : `${Math.round(rate * 100)}%`);
const long = (job) => job.p90 >= 60;
const plural = (n, what) => `${n} ${what}${n === 1 ? "" : "s"}`;
const days = (week) => Math.max(1, Math.round((Date.parse(week.to) - Date.parse(week.from)) / 86_400_000));

/** One line of the week's numbers, for the issue's comment and the PR that changes CI. */
export function headline(week) {
  const { runs, queue, flaky } = week;
  return [
    ...(week.ready && week.drops
      ? [
          `ready to merged p50 ${minutes(week.ready.p50)} min / p90 ${minutes(week.ready.p90)} min`,
          `queue drop rate ${percent(week.drops.rate)} (${week.drops.drops} of ${week.drops.entries} entries)`,
        ]
      : []),
    `${runs.runs} runs (${(runs.runs / days(week)).toFixed(1)}/day)`,
    `${percent(runs.successRate)} green`,
    `${runs.greenOnRerun} green only on a rerun`,
    `merge queue p50 ${minutes(queue.p50)} min / p90 ${minutes(queue.p90)} min`,
    plural(flaky.length, "flaky test"),
  ].join(", ");
}

/** The whole report, as Markdown for the job summary and the issue's body. */
export function report(week) {
  const { runs, queue, jobs, flaky, slowest, sampled, red, drift } = week;
  const lines = [
    `## CI health, ${week.from} to ${week.to}`,
    "",
    `**${headline(week)}**`,
    "",
    ...(week.unread > 0
      ? [`> [!WARNING]`, `> ${week.unread} of the week's runs could not be read: the runs list held them back. The numbers below are of the rest.`, ""]
      : []),
    ...(drift && drift.missing.length + drift.extra.length > 0
      ? [
          `> [!WARNING]`,
          `> \`tools/ci-push-jobs.mjs\` and main's required checks have parted, so its check reads the wrong list. ` +
            `Main requires and it does not know: ${drift.missing.join(", ") || "none"}. ` +
            `It lists and main does not require: ${drift.extra.join(", ") || "none"}.`,
          "",
        ]
      : []),
    "### Runs",
    "",
    "| day | runs |",
    "| --- | ---: |",
    ...Object.entries(runs.perDay)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([day, n]) => `| ${day} | ${n} |`),
    "",
    `Conclusions: ${Object.entries(runs.conclusions).map(([c, n]) => `${c} ${n}`).join(", ")}. ` +
      `Green is success over success and failure; a cancelled run (superseded by a newer push, or ` +
      `by the merge queue) counts toward neither.`,
    "",
    "A green run's wall time, from its last attempt's start to its end, in minutes:",
    "",
    "| event | runs | p50 | p90 |",
    "| --- | ---: | ---: | ---: |",
    ...Object.entries(runs.wall).map(([event, s]) => `| ${event} | ${s.n} | ${minutes(s.p50)} | ${minutes(s.p90)} |`),
    "",
    "### Merge queue",
    "",
    `${queue.merged} pull requests merged, ${queue.n} through the queue: from the last time each entered ` +
      `the queue to its merge, p50 ${minutes(queue.p50)} min, p90 ${minutes(queue.p90)} min.`,
    "",
    `Put out of the queue unmerged during the week, among the pull requests merged by its end: ${
      Object.entries(queue.ejected).map(([why, n]) => `${why}: ${n}`).join(", ") || "never"
    }. One put out and not merged by then is not counted.`,
    "",
    `### Jobs`,
    "",
    `Read from ${sampled.runs} of the week's ${sampled.of} runs, spread evenly over it${
      (sampled.others ?? []).length > 0
        ? `, and every run of ${sampled.others.map((one) => `\`${one.name}.yml\` (${one.runs})`).join(", ")}`
        : ""
    }. Minutes, slowest p90 first:`,
    "",
    "| job | runs | p50 | p90 |",
    "| --- | ---: | ---: | ---: |",
    ...jobs.filter(long).map((job) => `| ${job.name} | ${job.n} | ${minutes(job.p50)} | ${minutes(job.p90)} |`),
    "",
    // The required-check gates and the jobs a change's paths skip take seconds: listed, they bury the rest.
    ...(jobs.some((job) => !long(job))
      ? [`${plural(jobs.filter((job) => !long(job)).length, "more job")} took under a minute at p90.`, ""]
      : []),
    ...(week.over
      ? [
          "### Budgets",
          "",
          week.over.length === 0
            ? `No job is more than ${Math.round(OVER * 100)}% over its budget in \`ci/job-budgets.toml\`.`
            : `Jobs whose p50 of the week is more than ${Math.round(OVER * 100)}% over their budget in \`ci/job-budgets.toml\`, in minutes:`,
          "",
          ...(week.over.length === 0
            ? []
            : [
                "| job | p50 | budget | over |",
                "| --- | ---: | ---: | ---: |",
                ...week.over.map((job) => `| ${job.id} | ${job.p50.toFixed(1)} | ${job.budget} | +${Math.round(job.over * 100)}% |`),
              ]),
          "",
        ]
      : []),
    "### Red on a green run",
    "",
    `Jobs \`continue-on-error\` kept from turning the run red, read from ${red.runs} of main's green push and ` +
      `nightly runs: each is evidence someone should read, and stops nothing.`,
    "",
    ...(red.jobs.length === 0
      ? ["None."]
      : ["| times | job |", "| ---: | --- |", ...red.jobs.map((one) => `| ${one.times} | ${one.name} |`)]),
    "",
    "### Flaky tests",
    "",
    `Every \`Flaky (passed on retry N)\` warning in those ${sampled.runs} runs, by test:`,
    "",
    ...(flaky.length === 0
      ? ["None."]
      : ["| times | test |", "| ---: | --- |", ...flaky.slice(0, 100).map((one) => `| ${one.count} | \`${one.test}\` |`)]),
    "",
    "### Slowest tests",
    "",
    `From the "Slowest Rust tests" notices of the newest ${week.slowestFrom} green runs on main: each test's longest time.`,
    "",
    "| seconds | runs | test |",
    "| ---: | ---: | --- |",
    ...slowest.map((one) => `| ${one.seconds.toFixed(1)} s | ${one.runs} | \`${one.test}\` |`),
    "",
  ];
  return lines.join("\n");
}

// ---------------------------------------------------------------------------------------------
// Reading the API: everything above is pure and tested on recorded answers; below is the I/O.
//
//   GITHUB_TOKEN=… node tools/ci-health.mjs [--days 7] [--sample 600] [--issue]
//
// writes the report to $GITHUB_STEP_SUMMARY (or stdout), and with `--issue` into the one issue.
//
// The calls are bounded. `ci` runs ~400 times a day (488 on 2026-10-27), so the week's runs list
// is ~30 pages of 100, asked day by day because a filtered runs list answers at most 1,000; a day
// of more is read in slices (`runWindows`), and any run still not read is named in the report. Each
// run's jobs and their annotations come from GraphQL, ten check suites a query (more and GitHub
// refuses it as too costly), for `--sample` runs spread evenly over the week: 600 is ~60 queries,
// about four minutes, ~400 of GraphQL's 5,000 points an hour. The slowest tests are read from the
// newest SLOWEST_RUNS green runs on main, four rust jobs each. The merged pull requests are ~5
// pages of 50. Every green nightly and EVIDENCE_PUSHES green push runs on main are read for jobs
// red on a green run, ~7 queries, and main's required checks are two calls. ~120 calls in all.

/** How many of the newest green runs on main the slowest tests are read from. */
export const SLOWEST_RUNS = 3;

/** How many of the week's green push runs on main are read for jobs red on a green run. */
export const EVIDENCE_PUSHES = 60;

const API = process.env.GITHUB_API_URL ?? "https://api.github.com";

async function call(path, { method = "GET", body, token } = {}) {
  for (let attempt = 1; ; attempt += 1) {
    const response = await fetch(path.startsWith("http") ? path : `${API}${path}`, {
      method,
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        ...(body ? { "Content-Type": "application/json" } : {}),
      },
      body: body ? JSON.stringify(body) : undefined,
      signal: AbortSignal.timeout(60_000),
    });
    if (response.ok) return response.json();
    const text = await response.text();
    // A 502 or a secondary rate limit passes; anything else is the call's own fault.
    if (attempt < 3 && (response.status >= 500 || response.status === 403 || response.status === 429)) {
      await new Promise((resolve) => setTimeout(resolve, attempt * 10_000));
      continue;
    }
    throw new Error(`${method} ${path}: ${response.status} ${text.slice(0, 300)}`);
  }
}

async function graphql(query, variables, token) {
  const answer = await call("/graphql", { method: "POST", body: { query, variables }, token });
  if (answer.errors) throw new Error(`graphql: ${JSON.stringify(answer.errors).slice(0, 300)}`);
  return answer.data;
}

const SUITES = `query($ids: [ID!]!) {
  nodes(ids: $ids) { ... on CheckSuite {
    workflowRun { databaseId url createdAt }
    checkRuns(first: 100, filterBy: { checkType: LATEST }) { nodes {
      databaseId name conclusion startedAt completedAt
      annotations(first: 50) { nodes { annotationLevel title } }
    } }
  } }
}`;

const MERGED = `query($q: String!, $after: String) {
  search(type: ISSUE, query: $q, first: 50, after: $after) {
    pageInfo { hasNextPage endCursor }
    nodes { ... on PullRequest {
      number createdAt mergedAt
      timelineItems(last: 30, itemTypes: [ADDED_TO_MERGE_QUEUE_EVENT, REMOVED_FROM_MERGE_QUEUE_EVENT, READY_FOR_REVIEW_EVENT]) { nodes {
        __typename
        ... on ReadyForReviewEvent { createdAt }
        ... on AddedToMergeQueueEvent { createdAt }
        ... on RemovedFromMergeQueueEvent { createdAt reason }
      } }
    } }
  }
}`;

/** The check suites of `nodeIds`, ten a query, halving a batch GitHub finds too costly. */
async function suitesOf(nodeIds, token) {
  const out = [];
  const fetchBatch = async (ids) => {
    try {
      out.push(...(await graphql(SUITES, { ids }, token)).nodes);
    } catch (error) {
      if (ids.length === 1) throw error;
      const half = Math.ceil(ids.length / 2);
      await fetchBatch(ids.slice(0, half));
      await fetchBatch(ids.slice(half));
    }
  };
  for (let i = 0; i < nodeIds.length; i += 10) await fetchBatch(nodeIds.slice(i, i + 10));
  return out;
}

const isoDay = (ms) => new Date(ms).toISOString().slice(0, 10);

/**
 * The budgets' view of `release.yml`: the completed runs of the week, from `since` to `until` (UTC
 * days), and their check suites. The nightly profile guards are listed on their own (there are
 * seven a week), so a busy week of dev builds on push, of which the newest hundred are read, never
 * pushes them out of the `release` profile's budgets. A failed read is a warning and no release
 * verdict: it must not take the ci report down with it.
 */
async function releaseWeek({ repo, token, since, until }) {
  try {
    const list = async (event) =>
      (
        await call(
          `/repos/${repo}/actions/workflows/release.yml/runs?created=${since}..${until}&event=${event}&status=completed&per_page=100`,
          { token },
        )
      ).workflow_runs;
    const runs = [...(await list("schedule")), ...(await list("push"))].filter(
      (run) => run.conclusion === "success" || run.conclusion === "failure",
    );
    return { runs, suites: await suitesOf(runs.map((run) => run.check_suite_node_id), token) };
  } catch (error) {
    console.log(`::warning title=ci-health::release.yml budgets not read: ${error.message}`);
    return { runs: [], suites: [] };
  }
}

/**
 * What the weeks of `JOB_WORKFLOWS` add to ci's (`side`, one `{ name, runs, suites }` each, as
 * `workflowWeek` reads them): every suite, for the job durations and the flaky tests; the suites of
 * main's green runs, for the jobs red on a green run; and the budgets of each workflow's jobs
 * judged on its own runs (`jobsOf`, `{ [workflow]: jobs }`).
 */
export function sideWeek(side, budgets, jobsOf) {
  const onMain = (run) => run.head_branch === "main" && run.conclusion === "success";
  const green = new Set(side.flatMap((week) => week.runs.filter(onMain).map((run) => run.id)));
  return {
    suites: side.flatMap((week) => week.suites),
    green: side.flatMap((week) => week.suites.filter((suite) => green.has(suite?.workflowRun?.databaseId))),
    over: side.flatMap((week) => overBudget(week.suites, budgets, jobsOf[week.name] ?? [], week.name)),
  };
}

/**
 * The completed runs of `.github/workflows/<name>.yml` from `since` to `until` (UTC days), on main's
 * push and the nightly, and their check suites. A failed read is a warning and leaves that workflow
 * out: it must not take the ci report down with it.
 */
async function workflowWeek({ repo, token, name, since, until }) {
  try {
    const list = async (event) =>
      (
        await call(
          `/repos/${repo}/actions/workflows/${name}.yml/runs?created=${since}..${until}&event=${event}&status=completed&per_page=100`,
          { token },
        )
      ).workflow_runs;
    const runs = [...(await list("schedule")), ...(await list("push"))].filter(
      (run) => run.conclusion === "success" || run.conclusion === "failure",
    );
    return { name, runs, suites: await suitesOf(runs.map((run) => run.check_suite_node_id), token) };
  } catch (error) {
    console.log(`::warning title=ci-health::${name}.yml not read: ${error.message}`);
    return { name, runs: [], suites: [] };
  }
}

async function measure({ repo, token, dayCount, sampleSize }) {
  // Whole UTC days, ending yesterday: today's runs are still running.
  const today = Date.parse(isoDay(Date.now()));
  const dayList = Array.from({ length: dayCount }, (_, i) => isoDay(today - (dayCount - i) * 86_400_000));
  const runs = [];
  let unread = 0;
  const list = (created, page) =>
    call(`/repos/${repo}/actions/workflows/ci.yml/runs?created=${created}&per_page=100&page=${page}`, { token });
  for (const day of dayList) {
    const first = await list(day, 1);
    for (const created of runWindows(day, first.total_count)) {
      const read = [];
      let total = 0;
      for (let page = 1; page <= LIST_CAP / 100; page += 1) {
        const answer = created === day && page === 1 ? first : await list(created, page);
        total = answer.total_count;
        read.push(...answer.workflow_runs);
        if (answer.workflow_runs.length < 100) break;
      }
      // A run that started between two pages can push one onto the next: the id keeps it once.
      const fresh = read.filter((run, i) => read.findIndex((other) => other.id === run.id) === i);
      runs.push(...fresh);
      if (fresh.length < Math.min(total, LIST_CAP) || total > LIST_CAP) {
        unread += Math.max(0, total - fresh.length);
        console.log(`::warning title=ci-health::${created}: read ${fresh.length} of ${total} runs`);
      }
    }
  }
  runs.sort((a, b) => a.created_at.localeCompare(b.created_at));
  const completed = runs.filter((run) => run.status === "completed" && run.conclusion !== "skipped");
  const sampled = sample(completed, sampleSize);
  const mainGreen = completed
    .filter((run) => run.event === "push" && run.head_branch === "main" && run.conclusion === "success")
    .slice(-SLOWEST_RUNS);
  const suites = await suitesOf(sampled.map((run) => run.check_suite_node_id), token);
  const budgets = parseBudgets(readFileSync(BUDGETS_FILE, "utf8"));
  const release = await releaseWeek({ repo, token, since: dayList[0], until: dayList.at(-1) });
  const others = [];
  for (const name of Object.keys(JOB_WORKFLOWS)) {
    others.push(await workflowWeek({ repo, token, name, since: dayList[0], until: dayList.at(-1) }));
  }
  const side = sideWeek(others, budgets, workflowJobs());
  const mainSuites = await suitesOf(mainGreen.map((run) => run.check_suite_node_id), token);
  // The nightlies are few and the only runs of most evidence jobs, so all of them are read.
  const onMain = (run) => run.head_branch === "main" && run.conclusion === "success";
  const greenRuns = [
    ...completed.filter((run) => onMain(run) && run.event === "schedule"),
    ...sample(completed.filter((run) => onMain(run) && run.event === "push"), EVIDENCE_PUSHES),
  ];
  const greenSuites = await suitesOf(greenRuns.map((run) => run.check_suite_node_id), token);
  const required = requiredNames(
    await call(`/repos/${repo}/rules/branches/main?per_page=100`, { token }),
    (await call(`/repos/${repo}/branches/main`, { token })).protection,
  );
  const lists = [];
  for (const job of mainSuites.flatMap((suite) => suite?.checkRuns?.nodes ?? [])) {
    if (!job.name.startsWith("rust tests (part")) continue;
    lists.push(await call(`/repos/${repo}/check-runs/${job.databaseId}/annotations?per_page=100`, { token }));
  }
  const prs = [];
  let after = null;
  for (let page = 0; page < 20; page += 1) {
    const { search } = await graphql(
      MERGED,
      { q: `repo:${repo} is:pr is:merged merged:${dayList[0]}..${dayList.at(-1)}`, after },
      token,
    );
    prs.push(...search.nodes);
    if (!search.pageInfo.hasNextPage) break;
    after = search.pageInfo.endCursor;
  }
  return {
    from: dayList[0],
    to: dayList.at(-1),
    runs: runStats(runs),
    queue: queueStats(prs, { from: dayList[0], to: dayList.at(-1) }),
    ready: readyToMerged(prs),
    drops: queueDrops(prs, { from: dayList[0], to: dayList.at(-1) }),
    unread,
    sampled: { runs: sampled.length, of: runs.length, others: others.map((week) => ({ name: week.name, runs: week.runs.length })) },
    jobs: jobStats([...suites, ...side.suites]),
    over: [
      ...overBudget(suites, budgets, ciJobs(readFileSync(CI_FILE, "utf8"))),
      ...side.over,
      ...overReleaseBudget(release.runs, release.suites, budgets),
    ].sort((a, b) => b.over - a.over || a.id.localeCompare(b.id)),
    flaky: flakyTests([...suites, ...side.suites]),
    flakes: flakeEvidence([...suites, ...side.suites]),
    slowest: slowestTests(lists),
    slowestFrom: mainGreen.length,
    red: { runs: greenRuns.length + side.green.length, jobs: evidenceRed([...greenSuites, ...side.green]) },
    // No names means the token could not read them, which says nothing of the list.
    drift: required.length > 0 ? requiredDrift(required) : undefined,
  };
}

/** Every `area:ci` issue, any state, page by page: the report's own and the flake tickets. */
async function ciIssues({ repo, token }) {
  const all = [];
  for (let page = 1; page <= 100; page += 1) {
    const issues = await call(
      `/repos/${repo}/issues?labels=${encodeURIComponent(ISSUE_LABEL)}&state=all&per_page=100&page=${page}`,
      { token },
    );
    all.push(...issues.filter((issue) => !issue.pull_request));
    if (issues.length < 100) break;
  }
  return all;
}

/** Writes `body` into the report's one issue, made the first time, and comments `line` on it. */
async function keepIssue({ repo, token, body, line, issues }) {
  let issue = findIssue(issues);
  if (issue) {
    await call(`/repos/${repo}/issues/${issue.number}`, {
      method: "PATCH",
      body: { title: ISSUE_TITLE, body, state: "open" },
      token,
    });
  } else {
    issue = await call(`/repos/${repo}/issues`, {
      method: "POST",
      body: { title: ISSUE_TITLE, body, labels: [ISSUE_LABEL] },
      token,
    });
  }
  await call(`/repos/${repo}/issues/${issue.number}/comments`, { method: "POST", body: { body: line }, token });
  return issue.number;
}

/** One ticket per flaky test, and a comment on the ticket a test already has (`planFlakeTickets`). */
async function fileFlakes({ repo, token, flakes, issues }) {
  const { create, comment } = planFlakeTickets(
    flakes,
    issues.filter((issue) => !findIssue([issue])),
  );
  const created = [];
  for (const ticket of create) {
    created.push(await call(`/repos/${repo}/issues`, { method: "POST", body: ticket, token }));
  }
  for (const one of comment) {
    await call(`/repos/${repo}/issues/${one.number}/comments`, { method: "POST", body: { body: one.body }, token });
    await call(`/repos/${repo}/issues/${one.number}`, {
      method: "PATCH",
      body: { body: one.update, ...(one.reopen ? { state: "open" } : {}) },
      token,
    });
  }
  return { filed: create.length, commented: comment.length, created };
}

/** Makes the label `name` if the repository has none (a 422 says it has): safe to ask every day. */
async function ensureLabel({ repo, token, name }) {
  try {
    await call(`/repos/${repo}/labels`, { method: "POST", body: { name, color: "d93f0b", description: "a quarantined test waiting 14 days or more" }, token });
  } catch (error) {
    if (!/: 422 /.test(error.message)) throw error;
  }
}

/**
 * Quarantine, kept up (`planQuarantine`, `quarantineVerdict`): a pull request for each test that
 * flaked three times this week, the priority label on a ticket 14 days quarantined, and the
 * verdict, which is red past ten. The pull requests are made through the API (a branch off the
 * default branch, one commit to `ci/quarantine.toml`), so the run needs no checkout it can push.
 */
async function keepQuarantine({ repo, token, flakes, issues, reportIssue, today }) {
  const entries = parseQuarantine(readFileSync(QUARANTINE_FILE, "utf8"));
  const verdict = quarantineVerdict(entries, { today });
  // The verdict and the labels first: a pull request that cannot be opened must not hide them.
  if (verdict.prioritise.length > 0) await ensureLabel({ repo, token, name: PRIORITY_LABEL });
  for (const one of verdict.prioritise) {
    await call(`/repos/${repo}/issues/${one.ticket}/labels`, { method: "POST", body: { labels: [one.label] }, token });
  }
  if (verdict.red) {
    const operator = process.env.OPERATOR ? `@${process.env.OPERATOR.replace(/^@/, "")} ` : "";
    await call(`/repos/${repo}/issues/${reportIssue}/comments`, {
      method: "POST",
      body: { body: `${operator}${verdict.message}` },
      token,
    });
    process.exitCode = 1;
  }
  const pulls = await call(`/repos/${repo}/pulls?state=open&per_page=100`, { token });
  const proposals = planQuarantine({
    flakes,
    entries,
    issues,
    openBranches: pulls.map((pull) => pull.head.ref),
    today,
  });
  let opened = 0;
  if (proposals.length > 0) {
    const { default_branch: base } = await call(`/repos/${repo}`, { token });
    for (const proposal of proposals) {
      try {
        const { object } = await call(`/repos/${repo}/git/ref/heads/${base}`, { token });
        const file = await call(`/repos/${repo}/contents/ci/quarantine.toml?ref=${base}`, { token });
        const text = appendEntry(Buffer.from(file.content, "base64").toString("utf8"), proposal);
        await call(`/repos/${repo}/git/refs`, {
          method: "POST",
          body: { ref: `refs/heads/${proposal.branch}`, sha: object.sha },
          token,
        });
        await call(`/repos/${repo}/contents/ci/quarantine.toml`, {
          method: "PUT",
          body: {
            message: `${proposal.title} (#${proposal.ticket})`,
            content: Buffer.from(text).toString("base64"),
            sha: file.sha,
            branch: proposal.branch,
          },
          token,
        });
        const pull = await call(`/repos/${repo}/pulls`, {
          method: "POST",
          body: { title: proposal.title, head: proposal.branch, base, body: proposal.body },
          token,
        });
        await call(`/repos/${repo}/issues/${pull.number}/labels`, { method: "POST", body: { labels: [ISSUE_LABEL] }, token });
        opened += 1;
      } catch (error) {
        // A branch left behind by a closed proposal answers 422 (it exists): that test waits for a
        // person. Any other refusal (a token that cannot open pull requests) is named, and the rest go on.
        const left = /git\/refs: 422 /.test(error.message);
        console.log(`::${left ? "notice" : "warning"} title=ci-health quarantine::${proposal.id}: ${error.message.slice(0, 200)}`);
        if (!left) process.exitCode = 1;
      }
    }
  }
  return { proposed: opened, prioritised: verdict.prioritise.length, red: verdict.red };
}

async function main(argv) {
  const args = parseArgs(argv);
  const token = process.env.GITHUB_TOKEN;
  if (!token) throw new Error("GITHUB_TOKEN is not set");
  const repo = process.env.GITHUB_REPOSITORY ?? "purlis/purlis";
  const week = await measure({
    repo,
    token,
    dayCount: args.days,
    sampleSize: args.sample,
  });
  const body = report(week);
  const out = process.env.GITHUB_STEP_SUMMARY;
  if (out) appendFileSync(out, `${body}\n`);
  else process.stdout.write(`${body}\n`);
  console.log(headline(week));
  console.log(
    `read ${week.runs.runs} runs, the jobs of ${week.sampled.runs}, ${week.queue.merged} merged pull requests ` +
      `(${week.queue.n} through the queue), the slowest tests of ${week.slowestFrom} runs on main`,
  );
  if (args.issue) {
    const server = process.env.GITHUB_SERVER_URL ?? "https://github.com";
    const run = process.env.GITHUB_RUN_ID ? ` ([run](${server}/${repo}/actions/runs/${process.env.GITHUB_RUN_ID}))` : "";
    const issues = await ciIssues({ repo, token });
    const number = await keepIssue({ repo, token, body, issues, line: `${week.from} to ${week.to}: ${headline(week)}${run}` });
    console.log(`the report is in #${number}`);
    const { filed, commented, created } = await fileFlakes({ repo, token, flakes: week.flakes, issues });
    console.log(`flake tickets: ${filed} filed, ${commented} commented on`);
    const { create: budgetTickets } = planBudgetTickets(week.over, issues);
    for (const ticket of budgetTickets) await call(`/repos/${repo}/issues`, { method: "POST", body: ticket, token });
    console.log(`budget tickets: ${budgetTickets.length} filed`);
    const kept = await keepQuarantine({
      repo,
      token,
      flakes: week.flakes,
      issues: [...issues, ...created],
      reportIssue: number,
      today: isoDay(Date.now()),
    });
    console.log(`quarantine: ${kept.proposed} proposed, ${kept.prioritised} labelled for priority${kept.red ? ", RED" : ""}`);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
