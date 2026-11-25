// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { BUDGETS_FILE, budgetProblems, ciJobs, jobIdOf, overBudget, overReleaseBudget, parseBudgets, releaseProfile } from "./job-budgets.mjs";

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

// ---- release.yml jobs (#7384, train R5) ----

const RELEASE = new URL("../.github/workflows/release.yml", import.meta.url);
const fixture = (name) => JSON.parse(readFileSync(new URL(`./ci-health-fixtures/${name}`, import.meta.url), "utf8"));
const recorded = (which) => ({
  runs: fixture(`release-runs-${which}.json`).workflow_runs,
  suites: fixture(`release-suites-${which}.graphql.json`).data.nodes,
});

const RELEASE_BUDGETS = `
[[job]]
id = "release-macos-dev"
workflow = "release"
profile = "dev-release"
check = "macOS (.app)"
run = 45069994116
p50 = 91

[[job]]
id = "release-macos-release"
workflow = "release"
profile = "release"
check = "macOS (.app, .dmg)"
run = 45069994116
p50 = 91
`;

test("a release budget names its workflow, profile, check run and the run it was measured from", () => {
  assert.deepEqual(parseBudgets(RELEASE_BUDGETS)[1], {
    id: "release-macos-release",
    workflow: "release",
    profile: "release",
    check: "macOS (.app, .dmg)",
    run: 45069994116,
    p50: 91,
  });
});

test("a release budget without its profile, check run or measured run is refused", () => {
  const jobs = ciJobs(YML);
  const bare = (drop) => parseBudgets(RELEASE_BUDGETS.split("\n").filter((line) => !line.startsWith(drop)).join("\n"));
  assert.match(budgetProblems(jobs, [...parseBudgets(BUDGETS), ...bare("run =")]).join("\n"), /"release-macos-dev" names no run it was measured from/);
  assert.match(budgetProblems(jobs, [...parseBudgets(BUDGETS), ...bare("check =")]).join("\n"), /"release-macos-dev" names no check run/);
  assert.match(budgetProblems(jobs, [...parseBudgets(BUDGETS), ...bare("profile =")]).join("\n"), /"release-macos-dev" has no profile/);
  assert.deepEqual(budgetProblems(jobs.slice(0, 2), [...parseBudgets(BUDGETS), ...parseBudgets(RELEASE_BUDGETS)]), []);
});

test("two release budgets for the same profile and check run are refused", () => {
  const twice = parseBudgets(`${RELEASE_BUDGETS}\n${RELEASE_BUDGETS.replaceAll("macos-release", "macos-copy")}`);
  assert.match(budgetProblems(ciJobs(YML).slice(0, 2), [...parseBudgets(BUDGETS), ...twice]).join("\n"), /"release-macos-copy" budgets release: macOS \(\.app, \.dmg\) a second time/);
});

test("a release budget does not count as the budget of a ci job, and ci runs do not read it", () => {
  const budgets = [...parseBudgets(BUDGETS), ...parseBudgets(RELEASE_BUDGETS)];
  assert.deepEqual(overBudget(withRuns("macOS (.app, .dmg)", 500, 6), budgets, ciJobs(YML)), []);
});

test("a run is of the profile its title says: a dev build is dev-release, a nightly guard or a tag is release, a hand-started one is neither", () => {
  assert.equal(releaseProfile("dev build of 1a2b3c"), "dev-release");
  assert.equal(releaseProfile("profile guard"), "release");
  assert.equal(releaseProfile("stable v0.5.2"), "release");
  assert.equal(releaseProfile("on demand, some-branch"), undefined);
});

// The seam's tests read their own budgets, so resetting a number in the checked-in file never breaks them.
const realBudgets = () =>
  parseBudgets(
    [
      ["macos-dev", "dev-release", "macOS (.app)", 91],
      ["macos-release", "release", "macOS (.app, .dmg)", 91],
      ["linux", "dev-release", "Linux (.deb, AppImage)", 35],
      ["linux-release", "release", "Linux (.deb, AppImage)", 37],
      ["runner-dev", "dev-release", "runner binary (linux-aarch64)", 11],
      ["runner", "release", "runner binary (linux-aarch64)", 11],
    ]
      .map(([id, profile, check, p50]) => `[[job]]\nid = "release-${id}"\nworkflow = "release"\nprofile = "${profile}"\ncheck = "${check}"\nrun = 1\np50 = ${p50}\n`)
      .join("\n"),
  );


test("a recorded week of release runs inside its budgets files nothing", () => {
  const { runs, suites } = recorded("under");
  assert.deepEqual(overReleaseBudget(runs, suites, realBudgets()), []);
});

test("a recorded week whose release-profile macOS build runs 30% over files that budget, and the dev budget beside it stays quiet", () => {
  const { runs, suites } = recorded("over");
  const over = overReleaseBudget(runs, suites, realBudgets());
  assert.deepEqual(
    over.map((job) => [job.id, job.workflow, job.profile, job.p50, job.budget, job.n]),
    [["release-macos-release", "release", "release", 120, 91, 5]],
  );
});

test("a dev build over its dev budget does not count against the release-profile budget, nor the other way round", () => {
  const dev = (title, minutes, i) => ({
    run: { id: i, display_title: title, event: "push", status: "completed", conclusion: "success" },
    suite: { workflowRun: { databaseId: i }, checkRuns: { nodes: [ran("macOS (.app)", minutes)] } },
  });
  const slow = Array.from({ length: 6 }, (_, i) => dev("dev build of abc", 140, i));
  const over = overReleaseBudget(slow.map((x) => x.run), slow.map((x) => x.suite), realBudgets());
  assert.deepEqual(over.map((job) => job.id), ["release-macos-dev"]);
  const stable = Array.from({ length: 6 }, (_, i) => dev("profile guard", 140, i));
  assert.deepEqual(overReleaseBudget(stable.map((x) => x.run), stable.map((x) => x.suite), realBudgets()), []);
});

test("the checked-in release budgets cover each build job and the runner binary, once per profile, each with its run", () => {
  const release = parseBudgets(readFileSync(BUDGETS_FILE, "utf8")).filter((budget) => budget.workflow === "release");
  const checks = ["macOS (.app)", "macOS (.app, .dmg)", "Linux (.deb, AppImage)", "runner binary (linux-aarch64)"];
  for (const profile of ["dev-release", "release"]) {
    const mine = release.filter((budget) => budget.profile === profile).map((budget) => budget.check);
    for (const check of checks.filter((check) => profile === "release" ? check !== "macOS (.app)" : check !== "macOS (.app, .dmg)")) {
      assert.ok(mine.includes(check), `${profile} has a budget for ${check}`);
    }
  }
  const yml = readFileSync(RELEASE, "utf8");
  assert.match(yml, /name: runner binary \(linux-aarch64\)/);
  assert.match(yml, /\$\{\{ format\('\{0\} \(\{1\}\{2\}\)', matrix\.platform, matrix\.made/);
});

// A job whose RUSTFLAGS name mold (`-fuse-ld=mold`) links with it wherever it compiles, so the
// step that installs mold has to run whenever the job does. `recorded` installed it only when
// `rust` was in scope, then ran `cargo test` for the whole-tree guards when it was not (every
// draft pull request): `collect2: fatal error: cannot find 'ld'` (#7240).
test("every job that links with mold installs it on every run of the job", () => {
  const lines = readFileSync(CI, "utf8").split("\n");
  const jobsAt = lines.findIndex((l) => l === "jobs:");
  const starts = [];
  for (let i = jobsAt + 1; i < lines.length; i++) if (/^ {2}[\w-]+:\s*$/.test(lines[i])) starts.push(i);
  let checked = 0;
  starts.forEach((start, n) => {
    const job = lines.slice(start, starts[n + 1] ?? lines.length);
    if (!job.some((l) => /^\s+RUSTFLAGS:.*-fuse-ld=mold/.test(l))) return;
    checked++;
    const id = lines[start].trim();
    const install = job.findIndex((l) => /^\s+packages:.*\bmold\b/.test(l));
    assert.notEqual(install, -1, `${id} links with mold and never installs it`);
    // The step's own lines: back to its `- ` and on to the next one.
    let from = install;
    while (from > 0 && !/^ {6}- /.test(job[from])) from--;
    let to = install;
    while (to < job.length && (to === install || !/^ {6}- /.test(job[to]))) to++;
    const step = job.slice(from, to);
    assert.ok(!step.some((l) => /^\s+if:/.test(l)), `${id} installs mold under an if: it links without it when the step is skipped`);
  });
  assert.ok(checked >= 5, `expected the five mold jobs, found ${checked}`);
});
