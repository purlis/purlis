// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The weekly CI health report, read from recorded API answers (`tools/ci-health-fixtures/`, cut
// down from real answers of 2026-10-27). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { parseQuarantine, problemsWith } from "./quarantine.mjs";

import {
  ISSUE_TITLE,
  evidenceRed,
  findIssue,
  flakeEvidence,
  flakyTests,
  PRIORITY_LABEL,
  planBudgetTickets,
  appendEntry,
  planFlakeTickets,
  planQuarantine,
  quarantineVerdict,
  ticketFor,
  queueDrops,
  readyToMerged,
  headline,
  jobStats,
  locateIssue,
  parseArgs,
  queueStats,
  report,
  runStats,
  runWindows,
  sample,
  sideWeek,
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

// ---- the daily run: flake tickets, and the headline it is judged by (spec #7324) ----

const flakeSuite = (runId, job, ...tests) => flakeSuiteAt(runId, undefined, job, ...tests);
const flakeSuiteAt = (runId, createdAt, job, ...tests) => ({
  workflowRun: { databaseId: runId, createdAt, url: `https://github.com/purlis/purlis/actions/runs/${runId}` },
  checkRuns: {
    nodes: [
      {
        name: job,
        conclusion: "SUCCESS",
        annotations: { nodes: tests.map((test) => ({ title: `Flaky (passed on retry 1): ${test}` })) },
      },
    ],
  },
});

test("a flake's evidence names the suite that ran it and each run it flaked in, once each", () => {
  const found = flakeEvidence([
    flakeSuiteAt(11, "2026-11-20T01:00:00Z", "web (part 2/4)", "src/a.test.ts > a"),
    flakeSuiteAt(12, "2026-11-21T01:00:00Z", "web (part 2/4)", "src/a.test.ts > a", "src/b.test.ts > b"),
  ]);
  assert.deepEqual(found, [
    {
      test: "src/a.test.ts > a",
      count: 2,
      jobs: ["web (part 2/4)"],
      runs: [
        { id: 11, url: "https://github.com/purlis/purlis/actions/runs/11", createdAt: "2026-11-20T01:00:00Z" },
        { id: 12, url: "https://github.com/purlis/purlis/actions/runs/12", createdAt: "2026-11-21T01:00:00Z" },
      ],
    },
    {
      test: "src/b.test.ts > b",
      count: 1,
      jobs: ["web (part 2/4)"],
      runs: [{ id: 12, url: "https://github.com/purlis/purlis/actions/runs/12", createdAt: "2026-11-21T01:00:00Z" }],
    },
  ]);
});

const flakes = flakeEvidence([flakeSuite(11, "rust tests (part 1 of 4)", "purlis-core::it a_test")]);

test("a new flake is filed as one area:ci ticket naming its test, its suite and its runs", () => {
  const { create, comment } = planFlakeTickets(flakes, []);
  assert.equal(comment.length, 0);
  assert.equal(create.length, 1);
  assert.equal(create[0].title, "Flaky test: purlis-core::it a_test");
  assert.deepEqual(create[0].labels, ["area:ci"]);
  assert.match(create[0].body, /purlis-core::it a_test/);
  assert.match(create[0].body, /rust tests \(part 1 of 4\)/);
  assert.match(create[0].body, /actions\/runs\/11/);
});

test("a flake already filed gets a comment on its ticket, never a second ticket", () => {
  const filed = planFlakeTickets(flakes, []).create[0];
  const again = flakeEvidence([flakeSuite(13, "rust tests (part 1 of 4)", "purlis-core::it a_test")]);
  const plan = planFlakeTickets(again, [{ number: 42, state: "open", title: filed.title, body: filed.body, labels: [{ name: "area:ci" }] }]);
  assert.equal(plan.create.length, 0);
  assert.equal(plan.comment.length, 1);
  assert.equal(plan.comment[0].number, 42);
  assert.match(plan.comment[0].body, /actions\/runs\/13/);
  assert.match(plan.comment[0].update, /11,13/);
});

test("a run a ticket already names is not said again", () => {
  const filed = planFlakeTickets(flakes, []).create[0];
  const plan = planFlakeTickets(flakes, [{ number: 42, state: "open", title: filed.title, body: filed.body, labels: [{ name: "area:ci" }] }]);
  assert.deepEqual(plan, { create: [], comment: [] });
});

test("a ticket is found by its test even when its title was edited, and a closed one is reopened", () => {
  const filed = planFlakeTickets(flakes, []).create[0];
  const again = flakeAt(13, "2026-11-25T00:00:00Z");
  const plan = planFlakeTickets(again, [{ number: 9, state: "closed", closed_at: "2026-11-24T00:00:00Z", title: "renamed by hand", body: filed.body, labels: [{ name: "area:ci" }] }]);
  assert.equal(plan.create.length, 0);
  assert.equal(plan.comment[0].number, 9);
  assert.equal(plan.comment[0].reopen, true);
});

// A flake counts against a ticket only if its run started after the ticket was last closed: a run
// before the close is history the fix already answered (spec #7324).
const CLOSED = "2026-11-24T10:00:00Z";
const closedTicket = (body, extra = {}) => ({ number: 7, state: "closed", closed_at: CLOSED, title: "Flaky test: purlis-core::it a_test", body, labels: [{ name: "area:ci" }], ...extra });
const flakeAt = (runId, createdAt) => flakeEvidence([flakeSuiteAt(runId, createdAt, "rust tests (part 1 of 4)", "purlis-core::it a_test")]);

test("a closed ticket and only runs from before its close: no reopen, no comment, no new ticket", () => {
  const filed = planFlakeTickets(flakes, []).create[0];
  const old = flakeAt(20, "2026-11-20T08:00:00Z");
  assert.deepEqual(planFlakeTickets(old, [closedTicket(filed.body)]), { create: [], comment: [] });
  // Even when the ticket's body names no run and its title was edited by hand.
  assert.deepEqual(planFlakeTickets(old, [closedTicket(filed.body, { title: "renamed" })]), { create: [], comment: [] });
});

test("a closed ticket and a run that started after its close: reopened, citing only that run", () => {
  const filed = planFlakeTickets(flakes, []).create[0];
  const both = flakeEvidence([
    flakeSuiteAt(20, "2026-11-20T08:00:00Z", "rust tests (part 1 of 4)", "purlis-core::it a_test"),
    flakeSuiteAt(21, "2026-11-24T16:00:00Z", "rust tests (part 1 of 4)", "purlis-core::it a_test"),
  ]);
  const plan = planFlakeTickets(both, [closedTicket(filed.body)]);
  assert.equal(plan.comment.length, 1);
  assert.equal(plan.comment[0].reopen, true);
  assert.match(plan.comment[0].body, /Flaked again in 1 run/);
  assert.match(plan.comment[0].body, /actions\/runs\/21/);
  assert.doesNotMatch(plan.comment[0].body, /actions\/runs\/20/);
});

test("a ticket reopened for a new run does not hear the pre-close runs the next day, nor count them for quarantine", () => {
  const filed = planFlakeTickets(flakes, []).create[0];
  const both = flakeEvidence([
    flakeSuiteAt(20, "2026-11-20T08:00:00Z", "rust tests (part 1 of 4)", "purlis-core::it a_test"),
    flakeSuiteAt(21, "2026-11-24T16:00:00Z", "rust tests (part 1 of 4)", "purlis-core::it a_test"),
  ]);
  const [one] = planFlakeTickets(both, [closedTicket(filed.body)]).comment;
  const reopened = { ...closedTicket(one.update), state: "open", closed_at: null };
  assert.deepEqual(planFlakeTickets(both, [reopened]), { create: [], comment: [] });
});

test("a closed ticket and a run with no start time is history, never a reopen", () => {
  const filed = planFlakeTickets(flakes, []).create[0];
  assert.deepEqual(planFlakeTickets(flakeAt(22, undefined), [closedTicket(filed.body)]), { create: [], comment: [] });
});

test("an open ticket still hears every run it does not name, whenever it started", () => {
  const filed = planFlakeTickets(flakes, []).create[0];
  const plan = planFlakeTickets(flakeAt(20, "2026-11-01T00:00:00Z"), [closedTicket(filed.body, { state: "open", closed_at: null })]);
  assert.equal(plan.comment.length, 1);
  assert.equal(plan.comment[0].reopen, false);
});

const prAt = (number, createdAt, mergedAt, ...events) => ({
  number,
  createdAt,
  mergedAt,
  timelineItems: { nodes: events.map(([type, at, reason]) => ({ __typename: type, createdAt: at, reason })) },
});
const ready = (at) => ["ReadyForReviewEvent", at];
const added = (at) => ["AddedToMergeQueueEvent", at];
const removed = (at, reason) => ["RemovedFromMergeQueueEvent", at, reason];

test("ready to merged runs from the last time a pull request was marked ready, or from its opening", () => {
  const prs = [
    prAt(1, "2026-11-15T00:00:00Z", "2026-11-15T01:00:00Z"), // never a draft: 60 min
    prAt(2, "2026-11-15T00:00:00Z", "2026-11-15T10:00:00Z", ready("2026-11-15T05:00:00Z"), ready("2026-11-15T08:00:00Z")), // 120 min
    prAt(3, "2026-11-15T00:00:00Z", "2026-11-15T05:00:00Z", ready("2026-11-15T00:00:00Z")), // 300 min
  ];
  assert.deepEqual(readyToMerged(prs), { n: 3, p50: 120 * 60, p90: 300 * 60 });
});

test("the queue drop rate counts only failed_checks, over every entry, on recorded reasons", () => {
  const recorded = fixture("queue-reasons.graphql.json").data.search.nodes;
  // four entries; a merge conflict builds no queue run, so only the first removal is a drop
  assert.deepEqual(queueDrops(recorded), { entries: 4, drops: 1, rate: 0.25 });
});

test("no entries is no rate", () => {
  assert.deepEqual(queueDrops([]), { entries: 0, drops: 0, rate: null });
});

test("the daily headline leads with ready to merged and the queue drop rate", () => {
  const text = headline({ ...week, ready: { n: 173, p50: 291 * 60, p90: 1655 * 60 }, drops: { entries: 131, drops: 20, rate: 20 / 131 } });
  assert.match(text, /^ready to merged p50 291\.0 min \/ p90 1655\.0 min, queue drop rate 15% \(20 of 131 entries\), 6 runs/);
});

// ---- quarantine: proposals, the cap, and the 14-day escalation (#7333) ----

const flakedIn = (test, ...runIds) =>
  flakeEvidence(runIds.map((id) => flakeSuite(id, "web (part 1/4)", test))).find((one) => one.test === test);
const flakedAt = (test, ...times) =>
  flakeEvidence(times.map((at, i) => flakeSuiteAt(i + 1, at, "web (part 1/4)", test))).find((one) => one.test === test);
const ticketed = (...tests) => tests.map((test, i) => ({ number: 100 + i, state: "open", title: `Flaky test: ${test}`, body: "", labels: [{ name: "area:ci" }] }));
const TODAY = "2026-11-22";

test("runs from before a ticket's close do not count toward quarantine; three after it do", () => {
  const issues = [{ ...ticketed("src/a.test.ts > a")[0], state: "closed", closed_at: "2026-11-24T10:00:00Z" }];
  const old = flakedAt("src/a.test.ts > a", "2026-11-20T00:00:00Z", "2026-11-21T00:00:00Z", "2026-11-22T00:00:00Z");
  assert.deepEqual(planQuarantine({ flakes: [old], entries: [], issues, openBranches: [], today: TODAY }), []);
  const fresh = flakedAt("src/a.test.ts > a", "2026-11-25T00:00:00Z", "2026-11-26T00:00:00Z", "2026-11-27T00:00:00Z");
  assert.equal(planQuarantine({ flakes: [fresh], entries: [], issues, openBranches: [], today: TODAY }).length, 1);
});

test("a test that flaked in three runs of the week is proposed for quarantine, one that flaked in two is not", () => {
  const flaky = [flakedIn("src/a.test.ts > a", 1, 2, 3), flakedIn("src/b.test.ts > b", 4, 5)];
  const proposals = planQuarantine({ flakes: flaky, entries: [], issues: ticketed("src/a.test.ts > a", "src/b.test.ts > b"), openBranches: [], today: TODAY });
  assert.equal(proposals.length, 1);
  assert.deepEqual(
    { id: proposals[0].id, ticket: proposals[0].ticket, added: proposals[0].added },
    { id: "src/a.test.ts > a", ticket: 100, added: TODAY },
  );
  assert.match(proposals[0].branch, /^ci-health\/quarantine-[0-9a-f]{12}$/);
  assert.match(proposals[0].title, /Quarantine src\/a\.test\.ts > a/);
  assert.match(proposals[0].body, /#100/);
});

test("a flake already in the quarantine, or with its proposal already open, is not proposed again", () => {
  const flaky = [flakedIn("src/a.test.ts > a", 1, 2, 3)];
  const issues = ticketed("src/a.test.ts > a");
  const entries = [{ id: "src/a.test.ts > a", ticket: 100, added: "2026-11-01" }];
  assert.deepEqual(planQuarantine({ flakes: flaky, entries, issues, openBranches: [], today: TODAY }), []);
  const [open] = planQuarantine({ flakes: flaky, entries: [], issues, openBranches: [], today: TODAY });
  assert.deepEqual(planQuarantine({ flakes: flaky, entries: [], issues, openBranches: [open.branch], today: TODAY }), []);
});

test("no proposal is made once the quarantine is full", () => {
  const full = Array.from({ length: 10 }, (_, i) => ({ id: `t${i} x`, ticket: 400 + i, added: TODAY }));
  const flaky = [flakedIn("src/a.test.ts > a", 1, 2, 3)];
  assert.deepEqual(planQuarantine({ flakes: flaky, entries: full, issues: ticketed("src/a.test.ts > a"), openBranches: [], today: TODAY }), []);
});

test("a flake whose ticket is not filed yet waits for the next run", () => {
  assert.deepEqual(planQuarantine({ flakes: [flakedIn("src/a.test.ts > a", 1, 2, 3)], entries: [], issues: [], openBranches: [], today: TODAY }), []);
});

test("a ticket is found for a flake by its test id, as the flake tickets are", () => {
  assert.equal(ticketFor("src/a.test.ts > a", ticketed("src/a.test.ts > a"))?.number, 100);
  assert.equal(ticketFor("src/zzz.test.ts > z", ticketed("src/a.test.ts > a")), undefined);
});

test("a proposal's entry is added to the list in a form the quarantine reads back", () => {
  const text = appendEntry("# the list\n", { id: 'src/a.test.ts > says "hi"', ticket: 100, added: TODAY });
  const [entry] = parseQuarantine(text);
  assert.deepEqual(entry, { id: 'src/a.test.ts > says "hi"', ticket: 100, added: TODAY });
  assert.deepEqual(problemsWith(parseQuarantine(appendEntry(text, { id: "b c", ticket: 101, added: TODAY }))), []);
});

const entriesOf = (n, added = "2026-11-20") => Array.from({ length: n }, (_, i) => ({ id: `t${i} x`, ticket: 200 + i, added }));

test("more than ten quarantined tests is a red verdict, and ten is not", () => {
  assert.equal(quarantineVerdict(entriesOf(11), { today: TODAY }).red, true);
  assert.match(quarantineVerdict(entriesOf(11), { today: TODAY }).message, /11 tests are quarantined/);
  assert.equal(quarantineVerdict(entriesOf(10), { today: TODAY }).red, false);
});

test("an entry 14 days old has its ticket labelled for priority, one 13 days old does not", () => {
  const entries = [
    { id: "old x", ticket: 300, added: "2026-11-08" },
    { id: "young x", ticket: 301, added: "2026-11-09" },
  ];
  assert.deepEqual(quarantineVerdict(entries, { today: TODAY }).prioritise, [{ ticket: 300, label: PRIORITY_LABEL, days: 14 }]);
});

// ---- job budgets: a ticket for a job over its budget (#7334) ----

const slow = { id: "rust", p50: 37.5, budget: 30, over: 0.25, n: 40 };

test("a job 25% over its budget is filed as one area:ci ticket naming the job, its p50 and its budget", () => {
  const { create } = planBudgetTickets([slow], []);
  assert.equal(create.length, 1);
  assert.equal(create[0].title, "Job over budget: rust");
  assert.deepEqual(create[0].labels, ["area:ci"]);
  assert.match(create[0].body, /rust/);
  assert.match(create[0].body, /37\.5 min/);
  assert.match(create[0].body, /budget of 30 min/);
  assert.match(create[0].body, /ci\/job-budgets\.toml/);
});

test("a job whose budget ticket is still open gets no second one, and a closed ticket does not hold it", () => {
  const filed = planBudgetTickets([slow], []).create[0];
  const open = { number: 5, state: "open", title: filed.title, body: filed.body, labels: [{ name: "area:ci" }] };
  assert.deepEqual(planBudgetTickets([slow], [open]), { create: [] });
  assert.equal(planBudgetTickets([slow], [{ ...open, state: "closed" }]).create.length, 1);
});

test("the report names each job over its budget, and says when none is", () => {
  assert.match(report({ ...week, over: [slow] }), /### Budgets[\s\S]*\| rust \| 37\.5 \| 30 \| \+25% \|/);
  assert.match(report({ ...week, over: [] }), /### Budgets[\s\S]*No job is more than 20% over/);
  assert.doesNotMatch(report(week), /### Budgets/);
});

test("a release job over its budget is filed under release.yml and its profile, apart from a ci job of the same name", () => {
  const job = { id: "release-macos-release", workflow: "release", profile: "release", p50: 120.6, budget: 91, over: 0.325, n: 5 };
  const { create } = planBudgetTickets([job], []);
  assert.equal(create[0].title, "Job over budget: release-macos-release");
  assert.match(create[0].body, /job of `release\.yml`/);
  assert.match(create[0].body, /`release` profile/);
  assert.doesNotMatch(planBudgetTickets([slow], []).create[0].body, /profile/);
  assert.match(planBudgetTickets([slow], []).create[0].body, /job of `ci\.yml`/);
});

// The macOS outcome bars left ci.yml for outcome-bars.yml (ADR 0127, amended 2026-11-26), and
// ci-health still reads them: their budget on their own runs, and their red on main's green runs.
test("a workflow beside ci is read for its budgets on its own runs, and for its red on main's green runs", () => {
  const bars = "outcome bars (macos-latest) (macos-latest, bars, 1, false, false, false, true)";
  const at = (id, conclusion, minutes = 60) => ({
    workflowRun: { databaseId: id, url: `https://example.test/runs/${id}`, createdAt: "2026-11-25T10:00:00Z" },
    checkRuns: {
      nodes: [
        {
          name: bars,
          conclusion,
          startedAt: "2026-11-25T10:00:00Z",
          completedAt: new Date(Date.parse("2026-11-25T10:00:00Z") + minutes * 60_000).toISOString(),
        },
      ],
    },
  });
  const ids = [1, 2, 3, 4, 5, 6, 7];
  const weeks = [
    {
      name: "outcome-bars",
      runs: ids.map((id) => ({ id, head_branch: id === 7 ? "other" : "main", conclusion: "success" })),
      suites: ids.map((id) => at(id, id <= 2 ? "FAILURE" : "SUCCESS")),
    },
  ];
  const budgets = [{ id: "outcome-bars", workflow: "outcome-bars", p50: 40 }];
  const jobsOf = { "outcome-bars": [{ id: "outcome-bars", name: "outcome bars (macos-latest)" }] };
  const side = sideWeek(weeks, budgets, jobsOf);
  assert.equal(side.suites.length, 7);
  // Run 7 is not main's, so it is no evidence of main's red.
  assert.equal(side.green.length, 6);
  assert.deepEqual(evidenceRed(side.green), [{ name: bars, times: 2 }]);
  assert.deepEqual(
    side.over.map(({ id, workflow, p50, n }) => ({ id, workflow, p50, n })),
    [{ id: "outcome-bars", workflow: "outcome-bars", p50: 60, n: 5 }],
  );
  // A budget for ci's job of the same id is not judged on these runs.
  assert.deepEqual(sideWeek(weeks, [{ id: "outcome-bars", p50: 40 }], jobsOf).over, []);
  assert.match(planBudgetTickets(side.over, []).create[0].body, /The `outcome-bars` job of `outcome-bars\.yml` took a p50 of 60\.0 min/);
});

test("the report says which other workflows' runs its job table read", () => {
  assert.match(
    report({ ...week, sampled: { runs: 2, of: 6, others: [{ name: "outcome-bars", runs: 9 }] } }),
    /Read from 2 of the week's 6 runs, spread evenly over it, and every run of `outcome-bars\.yml` \(9\)\. Minutes/,
  );
  assert.match(report(week), /Read from 2 of the week's 6 runs, spread evenly over it\. Minutes/);
});
