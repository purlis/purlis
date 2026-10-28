// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The weekly CI health report (#3598): how the `ci` workflow did over the last seven days.
//
// CI got twice as slow and three times as flaky between 2026-10-14 and 10-27, and nobody saw it
// for two weeks. The `ci-health` workflow runs this every Monday and writes the week's numbers
// into its job summary and into one issue labelled `area:ci`, so the next slowdown shows on its
// first week: runs a day, how many went green, how many only on a rerun, the merge queue's wait,
// each job's p50 and p90, every flaky test by name, and the slowest tests.

import { appendFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** The `p` (0..1) percentile of `values` by nearest rank, or null for none. */
export function percentile(values, p) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(p * sorted.length) - 1)];
}

const seconds = (from, to) => Math.round((Date.parse(to) - Date.parse(from)) / 1000);
const spread = (values) => ({ n: values.length, p50: percentile(values, 0.5), p90: percentile(values, 0.9) });

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
  const decided = (conclusions.success ?? 0) + (conclusions.failure ?? 0);
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
 * counted by its reason.
 */
export function queueStats(prs) {
  const waits = [];
  const ejected = {};
  for (const pr of prs) {
    let entered = null;
    for (const event of pr.timelineItems?.nodes ?? []) {
      if (event.__typename === "AddedToMergeQueueEvent") entered = event.createdAt;
      if (event.__typename === "RemovedFromMergeQueueEvent" && event.reason !== "merged") {
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

/** At most `max` of `items`, spread evenly over them; all of them when there are no more. */
export function sample(items, max) {
  if (items.length <= max) return items;
  return Array.from({ length: max }, (_, i) => items[Math.floor((i * items.length) / max)]);
}

/** The issue the report is kept in: one, found by this title and the `area:ci` label. */
export const ISSUE_TITLE = "CI health: the weekly report";
export const ISSUE_LABEL = "area:ci";

/** The report's issue among `issues` (`GET /issues`), or undefined when there is none yet. */
export function findIssue(issues) {
  return issues.find(
    (issue) => issue.title === ISSUE_TITLE && issue.labels.some((label) => (label.name ?? label) === ISSUE_LABEL),
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
    `${runs.runs} runs (${(runs.runs / days(week)).toFixed(1)}/day)`,
    `${percent(runs.successRate)} green`,
    `${runs.greenOnRerun} green only on a rerun`,
    `merge queue p50 ${minutes(queue.p50)} min / p90 ${minutes(queue.p90)} min`,
    `${flaky.length} flaky tests`,
  ].join(", ");
}

/** The whole report, as Markdown for the job summary and the issue's body. */
export function report(week) {
  const { runs, queue, jobs, flaky, slowest, sampled } = week;
  const lines = [
    `## CI health, ${week.from} to ${week.to}`,
    "",
    `**${headline(week)}**`,
    "",
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
    `Put out of the queue unmerged: ${
      Object.entries(queue.ejected).map(([why, n]) => `${why}: ${n}`).join(", ") || "never"
    }.`,
    "",
    `### Jobs`,
    "",
    `Read from ${sampled.runs} of the week's ${sampled.of} runs, spread evenly over it. Minutes, slowest p90 first:`,
    "",
    "| job | runs | p50 | p90 |",
    "| --- | ---: | ---: | ---: |",
    ...jobs.filter(long).map((job) => `| ${job.name} | ${job.n} | ${minutes(job.p50)} | ${minutes(job.p90)} |`),
    "",
    // The required-check gates and the jobs a change's paths skip take seconds: listed, they bury the rest.
    ...(jobs.some((job) => !long(job))
      ? [`${plural(jobs.filter((job) => !long(job)).length, "more job")} took under a minute at p90.`, ""]
      : []),
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
// is ~30 pages of 100, asked day by day because a filtered runs list answers at most 1,000. Each
// run's jobs and their annotations come from GraphQL, ten check suites a query (more and GitHub
// refuses it as too costly), for `--sample` runs spread evenly over the week: 600 is ~60 queries,
// about four minutes, ~400 of GraphQL's 5,000 points an hour. The slowest tests are read from the
// newest SLOWEST_RUNS green runs on main, four rust jobs each. The merged pull requests are ~5
// pages of 50. ~110 calls in all.

/** How many of the newest green runs on main the slowest tests are read from. */
export const SLOWEST_RUNS = 3;

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
      number mergedAt
      timelineItems(last: 20, itemTypes: [ADDED_TO_MERGE_QUEUE_EVENT, REMOVED_FROM_MERGE_QUEUE_EVENT]) { nodes {
        __typename
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

async function measure({ repo, token, dayCount, sampleSize }) {
  // Whole UTC days, ending yesterday: today's runs are still running.
  const today = Date.parse(isoDay(Date.now()));
  const dayList = Array.from({ length: dayCount }, (_, i) => isoDay(today - (dayCount - i) * 86_400_000));
  const runs = [];
  for (const day of dayList) {
    for (let page = 1; page <= 10; page += 1) {
      const answer = await call(
        `/repos/${repo}/actions/workflows/ci.yml/runs?created=${day}&per_page=100&page=${page}`,
        { token },
      );
      runs.push(...answer.workflow_runs);
      if (answer.workflow_runs.length < 100) break;
    }
  }
  runs.sort((a, b) => a.created_at.localeCompare(b.created_at));
  const completed = runs.filter((run) => run.status === "completed" && run.conclusion !== "skipped");
  const sampled = sample(completed, sampleSize);
  const mainGreen = completed
    .filter((run) => run.event === "push" && run.head_branch === "main" && run.conclusion === "success")
    .slice(-SLOWEST_RUNS);
  const suites = await suitesOf(sampled.map((run) => run.check_suite_node_id), token);
  const mainSuites = await suitesOf(mainGreen.map((run) => run.check_suite_node_id), token);
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
    queue: queueStats(prs),
    sampled: { runs: sampled.length, of: runs.length },
    jobs: jobStats(suites),
    flaky: flakyTests(suites),
    slowest: slowestTests(lists),
    slowestFrom: mainGreen.length,
  };
}

/** Writes `body` into the report's one issue, made the first time, and comments `line` on it. */
async function keepIssue({ repo, token, body, line }) {
  const issues = await call(
    `/repos/${repo}/issues?labels=${encodeURIComponent(ISSUE_LABEL)}&state=all&per_page=100`,
    { token },
  );
  let issue = findIssue(issues);
  if (issue) {
    await call(`/repos/${repo}/issues/${issue.number}`, { method: "PATCH", body: { body, state: "open" }, token });
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

async function main(argv) {
  const flag = (name, fallback) => {
    const at = argv.indexOf(name);
    return at >= 0 ? argv[at + 1] : fallback;
  };
  const token = process.env.GITHUB_TOKEN;
  if (!token) throw new Error("GITHUB_TOKEN is not set");
  const repo = process.env.GITHUB_REPOSITORY ?? "purlis/purlis";
  const week = await measure({
    repo,
    token,
    dayCount: Number(flag("--days", "7")),
    sampleSize: Number(flag("--sample", "600")),
  });
  const body = report(week);
  const out = process.env.GITHUB_STEP_SUMMARY;
  if (out) appendFileSync(out, `${body}\n`);
  else process.stdout.write(`${body}\n`);
  console.log(headline(week));
  if (argv.includes("--issue")) {
    const server = process.env.GITHUB_SERVER_URL ?? "https://github.com";
    const run = process.env.GITHUB_RUN_ID ? ` ([run](${server}/${repo}/actions/runs/${process.env.GITHUB_RUN_ID}))` : "";
    const number = await keepIssue({ repo, token, body, line: `${week.from} to ${week.to}: ${headline(week)}${run}` });
    console.log(`the report is in #${number}`);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
