// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The weekly CI health report, read from recorded API answers (`tools/ci-health-fixtures/`, cut
// down from real answers of 2026-10-27). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
  ISSUE_TITLE,
  evidenceRed,
  findIssue,
  flakyTests,
  headline,
  jobStats,
  locateIssue,
  parseArgs,
  queueStats,
  report,
  runStats,
  runWindows,
  sample,
  slowestTests,
} from "./ci-health.mjs";

const fixture = (name) =>
  JSON.parse(readFileSync(new URL(`./ci-health-fixtures/${name}`, import.meta.url), "utf8"));

const runs = fixture("runs.json").workflow_runs;

test("a job red on a green run is named, each time it was, with its time limit counted as red", () => {
  const job = (name, conclusion) => ({ name, conclusion });
  const suites = [
    { checkRuns: { nodes: [job("outcome bars (macos-latest)", "FAILURE"), job("rust", "SUCCESS")] } },
    { checkRuns: { nodes: [job("outcome bars (macos-latest)", "TIMED_OUT"), job("fedora", "FAILURE")] } },
    { checkRuns: { nodes: [job("skipped one", "SKIPPED"), job("cancelled one", "CANCELLED")] } },
    null,
  ];
  assert.deepEqual(evidenceRed(suites), [
    { name: "outcome bars (macos-latest)", times: 2 },
    { name: "fedora", times: 1 },
  ]);
});

test("runs are counted per day, and cancelled runs count toward neither success nor failure", () => {
  const stats = runStats(runs);
  assert.equal(stats.runs, 6);
  assert.deepEqual(stats.perDay, { "2026-10-27": 6 });
  assert.deepEqual(stats.conclusions, { success: 4, failure: 1, cancelled: 1 });
  assert.equal(stats.successRate, 0.8);
});

test("a run that timed out or never started counts as red, as a failure does", () => {
  const red = [
    { ...runs[2], id: 1, conclusion: "timed_out" },
    { ...runs[2], id: 2, conclusion: "startup_failure" },
  ];
  assert.equal(runStats([...runs, ...red]).successRate, 4 / 7);
});

test("a day of more runs than one list answers is read in windows small enough for it", () => {
  assert.deepEqual(runWindows("2026-10-27", 488), ["2026-10-27"]);
  assert.deepEqual(runWindows("2026-10-27", 1500), [
    "2026-10-27T00:00:00Z..2026-10-27T11:59:59Z",
    "2026-10-27T12:00:00Z..2026-10-27T23:59:59Z",
  ]);
});

test("a run that went green on its second attempt is counted as green only on a rerun", () => {
  assert.equal(runStats(runs).greenOnRerun, 1);
});

test("a green run's wall time is from its last attempt's start to its end, by event", () => {
  assert.deepEqual(runStats(runs).wall, {
    push: { n: 1, p50: 2444, p90: 2444 },
    pull_request: { n: 1, p50: 1449, p90: 1449 },
    merge_group: { n: 2, p50: 1454, p90: 1932 },
  });
});

const merged = fixture("merged-prs.graphql.json").data.search.nodes;

test("a pull request's queue time is from its last entry into the merge queue to its merge", () => {
  const queue = queueStats(merged);
  assert.equal(queue.merged, 4);
  assert.equal(queue.p50, 1770);
  assert.equal(queue.p90, 2015);
});

test("each time the queue put a pull request out unmerged is counted by why", () => {
  assert.deepEqual(queueStats(merged).ejected, { merge_conflict: 1 });
});

test("only the week's own ejections are counted, not those of a merged pull request's earlier days", () => {
  // #3577's one ejection was on 2026-10-28.
  assert.deepEqual(queueStats(merged, { from: "2026-10-21", to: "2026-10-27" }).ejected, {});
  assert.deepEqual(queueStats(merged, { from: "2026-10-22", to: "2026-10-28" }).ejected, { merge_conflict: 1 });
});

test("a pull request merged without the queue has no queue time", () => {
  const queue = queueStats([{ number: 1, mergedAt: "2026-10-27T10:00:00Z", timelineItems: { nodes: [] } }]);
  assert.deepEqual(queue, { merged: 1, n: 0, p50: null, p90: null, ejected: {} });
});

const suites = fixture("check-suites.graphql.json").data.nodes;

test("each job's duration is spread over the runs it ran in, slowest p90 first", () => {
  const jobs = jobStats(suites);
  assert.deepEqual(jobs[0], { name: "rust tests (part 1 of 3)", n: 1, p50: 1796, p90: 1796 });
  assert.deepEqual(
    jobs.find((job) => job.name === "app builds (macos-latest)"),
    { name: "app builds (macos-latest)", n: 2, p50: 312, p90: 472 },
  );
});

test("a skipped job has no duration", () => {
  assert.equal(
    jobStats(suites).find((job) => job.name.startsWith("windows")),
    undefined,
  );
});

test("a failed job's duration is not counted: it ended where it failed, not where it would have", () => {
  const failed = {
    checkRuns: {
      nodes: [{ name: "recorded behaviour", conclusion: "FAILURE", startedAt: "2026-10-27T10:00:00Z", completedAt: "2026-10-27T10:00:30Z" }],
    },
  };
  const recorded = jobStats([...suites, failed]).find((job) => job.name === "recorded behaviour");
  assert.deepEqual(recorded, { name: "recorded behaviour", n: 2, p50: 225, p90: 274 });
});

test("every test the rust jobs called flaky is counted by name", () => {
  assert.deepEqual(flakyTests(suites), [
    {
      test: "purlis-browser-engine::typing_while_driving_stays_quick_on_a_large_page a_key_into_a_second_field_of_a_large_page_is_held_in_words_until_the_page_is_read",
      count: 1,
    },
    {
      test: "purlis-host::the_kill_switch_reaches_every_runner_this_machine_keeps a_runner_not_reached_is_tried_again_while_the_desktop_stays_stopped",
      count: 1,
    },
  ]);
});

test("a test flaky in two runs is counted twice and listed first", () => {
  const twice = { title: "Flaky (passed on retry 1): purlis-core::a b", annotationLevel: "WARNING" };
  const suite = { checkRuns: { nodes: [{ name: "rust tests (part 1 of 4)", conclusion: "SUCCESS", annotations: { nodes: [twice] } }] } };
  assert.deepEqual(flakyTests([...suites, suite, suite])[0], { test: "purlis-core::a b", count: 2 });
});

const annotations = fixture("annotations.json");

test("the slowest tests are read from the rust jobs' notices, slowest first", () => {
  assert.deepEqual(slowestTests([annotations]), [
    { test: "purlis-core  theme::import_tests::a_light_scheme_s_hues_walk_toward_its_darker_words", seconds: 78.6, runs: 1 },
    { test: "purlis-core  theme::import_tests::selected_text_is_the_foreground_when_the_source_colours_only_the_selection", seconds: 61, runs: 1 },
    { test: "purlis-core  theme::import_tests::a_property_list_nested_deeper_than_a_preset_is_refused_before_it_is_parsed", seconds: 55, runs: 1 },
  ]);
});

test("a test slow in several runs keeps its longest time and counts the runs", () => {
  const again = [{ title: "Slowest Rust tests", message: "  80.0 s  purlis-core  theme::import_tests::a_light_scheme_s_hues_walk_toward_its_darker_words" }];
  const [first] = slowestTests([annotations, again], { limit: 1 });
  assert.deepEqual(first, {
    test: "purlis-core  theme::import_tests::a_light_scheme_s_hues_walk_toward_its_darker_words",
    seconds: 80,
    runs: 2,
  });
});

test("a sample is spread evenly over the week, and a short week is read whole", () => {
  const ten = Array.from({ length: 10 }, (_, i) => i);
  assert.deepEqual(sample(ten, 3), [0, 3, 6]);
  assert.deepEqual(sample(ten, 20), ten);
});

const week = {
  from: "2026-10-21",
  to: "2026-10-28",
  runs: runStats(runs),
  queue: queueStats(merged),
  sampled: { runs: 2, of: 6 },
  jobs: [...jobStats(suites), { name: "quick check", n: 2, p50: 5, p90: 6 }],
  flaky: flakyTests(suites),
  slowest: slowestTests([annotations]),
  slowestFrom: 1,
  red: { runs: 8, jobs: [{ name: "outcome bars (macos-latest)", times: 5 }] },
};

test("the report names a job red on a green run, and says when there were none", () => {
  assert.match(report(week), /Red on a green run[\s\S]*read from 8 of main's[\s\S]*\| 5 \| outcome bars \(macos-latest\) \|/);
  assert.match(report({ ...week, red: { runs: 8, jobs: [] } }), /Red on a green run[\s\S]*None\./);
});

test("the report warns when main's required checks and the push-job check's list have parted", () => {
  const drift = { missing: ["parity"], extra: [] };
  assert.match(report({ ...week, drift }), /\[!WARNING\][\s\S]*have parted[\s\S]*does not know: parity\./);
  assert.doesNotMatch(report({ ...week, drift: { missing: [], extra: [] } }), /have parted/);
  assert.doesNotMatch(report(week), /have parted/);
});

test("the report's headline names the numbers a slowdown shows in first", () => {
  assert.equal(
    headline(week),
    "6 runs (0.9/day), 80% green, 1 green only on a rerun, merge queue p50 29.5 min / p90 33.6 min, 2 flaky tests",
  );
});

test("one flaky test is named in the singular", () => {
  assert.match(headline({ ...week, flaky: week.flaky.slice(0, 1) }), /, 1 flaky test$/);
});

test("the report lists every flaky test with its count, and the jobs with their spread in minutes", () => {
  const text = report(week);
  assert.match(text, /^## CI health, 2026-10-21 to 2026-10-28$/m);
  assert.match(text, /^\| 1 \| `purlis-host::the_kill_switch_reaches_every_runner_this_machine_keeps a_runner_not_reached_is_tried_again_while_the_desktop_stays_stopped` \|$/m);
  assert.match(text, /^\| rust tests \(part 1 of 3\) \| 1 \| 29\.9 \| 29\.9 \|$/m);
  assert.match(text, /^\| 78\.6 s \| 1 \| `purlis-core  theme::import_tests::a_light_scheme_s_hues_walk_toward_its_darker_words` \|$/m);
  assert.match(text, /merge_conflict: 1/);
  assert.doesNotMatch(text, /\| quick check \|/);
  assert.match(text, /1 more job took under a minute at p90/);
  assert.match(text, /2 of the week's 6 runs/);
});

test("the report says how many runs it could not read, when the runs list held back some", () => {
  assert.match(report({ ...week, unread: 12 }), /12 of the week's runs could not be read/);
  assert.doesNotMatch(report(week), /could not be read/);
});

test("the report's one issue is looked for past the first page of area:ci issues", async () => {
  const pages = {
    1: Array.from({ length: 100 }, (_, i) => ({ number: 1000 + i, title: "other", labels: [{ name: "area:ci" }] })),
    2: [{ number: 7, title: ISSUE_TITLE, labels: [{ name: "area:ci" }] }],
  };
  const asked = [];
  const issue = await locateIssue(async (page) => {
    asked.push(page);
    return pages[page] ?? [];
  });
  assert.equal(issue.number, 7);
  assert.deepEqual(asked, [1, 2]);
});

test("no issue is found once the pages run out", async () => {
  assert.equal(await locateIssue(async (page) => (page === 1 ? [{ number: 1, title: "x", labels: [] }] : [])), undefined);
});

test("the sample and the days are positive whole numbers, and a flag is never taken for one", () => {
  assert.deepEqual(parseArgs([]), { days: 7, sample: 600, issue: false });
  assert.deepEqual(parseArgs(["--sample", "50", "--issue"]), { days: 7, sample: 50, issue: true });
  assert.throws(() => parseArgs(["--sample", "--issue"]), /--sample/);
  assert.throws(() => parseArgs(["--sample", "abc"]), /--sample/);
  assert.throws(() => parseArgs(["--sample", "0"]), /--sample/);
  assert.throws(() => parseArgs(["--days", "1.5"]), /--days/);
});

test("the report's one issue is found by its title and its label, open or not", () => {
  const issues = [
    { number: 1, title: ISSUE_TITLE, labels: [{ name: "area:app" }] },
    { number: 2, title: "something else", labels: [{ name: "area:ci" }] },
    { number: 3, title: ISSUE_TITLE, labels: [{ name: "area:ci" }] },
  ];
  assert.equal(findIssue(issues).number, 3);
  assert.equal(findIssue(issues.slice(0, 2)), undefined);
});
