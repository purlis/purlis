// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What the scenario legs' reports say across runs (M74.1, T1.1-54): the nightly's three runs, the
// stress run's twenty, the shuffle between them. The first test name is the ticket's.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { jobsOf } from "./ci-push-jobs.mjs";
import { batchOf, batchesChanged, capped, differences, escapeRows, filterOf, flaky, junitRecords, open, recordsOfSteps, shuffled } from "./scenario-legs.mjs";

const TOOL = fileURLToPath(new URL("./scenario-legs.mjs", import.meta.url));

const rec = (row, verdict, extra = {}) => ({
  kind: "cell",
  row,
  dialect: "claude",
  protection: "full",
  os: "linux",
  leg: "fake",
  timings: "scaled",
  verdict,
  ...extra,
});

/** A folder of runs: `runs[i]` is run i's records, written as one report file. */
function runsOf(runs) {
  const dir = mkdtempSync(join(tmpdir(), "legs-"));
  runs.forEach((records, i) => {
    mkdirSync(join(dir, `run-${i + 1}`));
    writeFileSync(join(dir, `run-${i + 1}`, "linux-fake.jsonl"), records.map((r) => JSON.stringify(r)).join("\n") + "\n");
  });
  return dir;
}

function cellsOf(batch) {
  const dir = mkdtempSync(join(tmpdir(), "cells-"));
  writeFileSync(join(dir, "B.json"), JSON.stringify(batch));
  return dir;
}

test("a_nightly_verdict_difference_opens_a_flaky_scenario_issue", () => {
  // Dry run: run 2 differs on M-NET-01 only.
  const dir = runsOf([
    [rec("M-NET-01", "pass"), rec("M-NET-02", "pass")],
    [rec("M-NET-01", "fail"), rec("M-NET-02", "pass")],
    [rec("M-NET-01", "pass"), rec("M-NET-02", "pass")],
  ]);
  try {
    const found = flaky(dir, { source: "nightly" });
    assert.equal(found.runs, 3);
    assert.equal(found.differences.length, 1);
    const [issue] = found.issues;
    assert.match(issue.title, /^Flaky scenario: M-NET-01/);
    assert.deepEqual(issue.labels, ["flaky-scenario", "bot-maintained"]);
    assert.match(issue.body, /run 1: pass; run 2: fail; run 3: pass/);
    assert.match(issue.body, /counts as failing/);

    // The command, dry: it prints the issue it would open, opens none, and is red.
    const ran = spawnSync("node", [TOOL, "flaky", dir, "--dry-run", "--cells", "/nonexistent"], { encoding: "utf8" });
    assert.equal(ran.status, 1, ran.stderr);
    const would = ran.stdout.split("\n").filter((l) => l.startsWith("{")).map((l) => JSON.parse(l));
    assert.equal(would.length, 1);
    assert.match(would[0].wouldOpen.title, /M-NET-01/);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test("runs_that_agree_open_no_issue_and_are_green", () => {
  const same = [rec("M-NET-01", "pass"), rec("M-NET-03", "expected_fail")];
  const dir = runsOf([same, same, same]);
  try {
    const ran = spawnSync("node", [TOOL, "flaky", dir, "--dry-run"], { encoding: "utf8" });
    assert.equal(ran.status, 0, ran.stdout + ran.stderr);
    assert.doesNotMatch(ran.stdout, /wouldOpen/);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test("an_entry_a_run_never_picked_up_differs_from_one_that_did", () => {
  const found = differences([[rec("M-A-01", "pass")], [], [rec("M-A-01", "pass")]]);
  assert.equal(found.length, 1);
  assert.deepEqual(found[0].verdicts, ["pass", "missing", "pass"]);
});

test("an_escape_whose_verdict_differs_opens_a_public_p0_issue_never_a_flaky_one", () => {
  const dir = runsOf([
    [rec("E-NET-01", "pass", { kind: "escape" }), rec("M-REF-01", "pass")],
    [rec("E-NET-01", "fail", { kind: "escape" }), rec("M-REF-01", "fail")],
  ]);
  // M-REF-01 is a cell, but one step expects a refusal: a refusal that did not hold.
  const cells = cellsOf({ cells: [{ id: "M-REF-01", steps: [{ refusal_expected: true }] }, { id: "M-NET-01", steps: [{}] }] });
  try {
    const found = flaky(dir, { cells, source: "stress-x20" });
    assert.equal(found.issues.length, 2);
    for (const issue of found.issues) {
      assert.deepEqual(issue.labels, ["area:security", "priority:p0", "bot-maintained"]);
      assert.match(issue.title, /^Escape verdict differs/);
      assert.match(issue.body, /stress-x20/);
    }
    assert.deepEqual([...escapeRows(cells)], ["M-REF-01"]);
  } finally {
    rmSync(dir, { recursive: true });
    rmSync(cells, { recursive: true });
  }
});

test("the_shuffle_moves_batches_and_never_the_cells_inside_one", () => {
  const names = [];
  for (const batch of ["b0", "b1", "b2", "b3", "b4", "b5"]) {
    for (const cell of ["m_01", "m_02", "m_03"]) names.push(`scenarios::${batch}::${cell}`);
  }
  assert.equal(batchOf("scenarios::b0::m_01"), "scenarios::b0");
  const one = shuffled(names, 1);
  assert.deepEqual([...one].sort(), [...new Set(names.map(batchOf))].sort(), "every batch, once");
  assert.deepEqual(shuffled(names, 1), one, "the same seed, the same order");
  const orders = new Set([1, 2, 3, 4, 5].map((seed) => shuffled(names, seed).join()));
  assert.ok(orders.size > 1, "a different seed moves the batches");
  const ran = spawnSync("node", [TOOL, "shuffle", "7"], { input: names.join("\n"), encoding: "utf8" });
  assert.equal(ran.status, 0, ran.stderr);
  assert.deepEqual(ran.stdout.trim().split("\n"), shuffled(names, 7));
});

test("the_stress_run_picks_the_batches_a_pull_request_changed", () => {
  const paths = ["tests/scenarios/cells/B3.json", "tests/scenarios/cells/B0.json", "tools/land.mjs", "tests/scenarios/coverage.json"];
  assert.deepEqual(batchesChanged(paths), ["B0", "B3"]);
  assert.equal(filterOf(batchesChanged(paths)), "test(/(^|::)(b0|b3)::/)");
  assert.equal(filterOf(batchesChanged(["docs/spec.md"])), "", "none changed: the whole suite");
});

const root = fileURLToPath(new URL("..", import.meta.url));

test("the_existing_macos_sandbox_targets_run_and_fail_on_red: nightly and on the macos-scenarios label, in the leg report", () => {
  // Each target is a step with an id; its outcome is its record, and a red or skipped one is never a pass.
  const records = recordsOfSteps(
    { "studio-probe": { outcome: "success" }, "ssh-route": { outcome: "failure" }, keychain: { outcome: "skipped" } },
    "existing-macos-sandbox-tests",
    "macos",
  );
  assert.deepEqual(records.map((r) => [r.test, r.verdict]), [
    ["studio-probe", "pass"],
    ["ssh-route", "fail"],
    ["keychain", "not_run"],
  ]);
  for (const r of records) assert.deepEqual([r.kind, r.leg, r.os], ["fixture", "fixture", "macos"]);
  assert.match(records[2].reason, /skipped/);

  // The action names each target's step and writes them after any red one.
  const action = readFileSync(join(root, ".github/actions/macos-sandbox-tests/action.yml"), "utf8");
  for (const id of ["studio-probe", "listens-nowhere", "server-behind-its-door", "ssh-route", "brokered-through-its-proxy", "hands-on-one-connection", "keychain-item", "purlis-app"]) {
    assert.match(action, new RegExp(`\\n      id: ${id}\\n`), id);
  }
  assert.match(action, /if: \$\{\{ always\(\) \}\}[\s\S]*toJSON\(steps\)[\s\S]*scenario-legs\.mjs steps existing-macos-sandbox-tests macos/);
  // The job uploads them with the scenarios' reports, nightly and on the label.
  const job = jobsOf(readFileSync(join(root, ".github/workflows/sandbox-scenarios.yml"), "utf8")).find((j) => j.id === "macos-sandbox-tests");
  assert.match(job.text, /sandbox-scenarios-report-existing-macos/);
  assert.match(job.text, /target\/sandbox-scenarios\/\*\.jsonl/);
  assert.match(readFileSync(join(root, ".github/nightly-on-phase-1-proven.txt"), "utf8"), /^existing-macos-sandbox-tests\s+macos-/m);
});

test("a_test_nextest_passed_in_one_run_and_failed_in_another_is_flaky_with_no_cell_report", () => {
  const xml = (verdicts) =>
    `<testsuites><testsuite name="purlis-app">${verdicts
      .map(([name, v]) => `<testcase name="${name}" classname="purlis-app"${v === "pass" ? "/>" : `>${v === "fail" ? "<failure message=\"x\"/>" : "<skipped/>"}</testcase>`}`)
      .join("")}</testsuite></testsuites>`;
  assert.deepEqual(
    junitRecords(xml([["scenarios::a", "pass"], ["scenarios::b", "fail"], ["scenarios::c", "skip"]])).map((r) => [r.row, r.verdict]),
    [["purlis-app scenarios::a", "pass"], ["purlis-app scenarios::b", "fail"], ["purlis-app scenarios::c", "not_run"]],
  );
  const dir = mkdtempSync(join(tmpdir(), "legs-"));
  try {
    for (const [i, v] of ["pass", "fail", "pass"].entries()) {
      mkdirSync(join(dir, `run-${i + 1}`));
      writeFileSync(join(dir, `run-${i + 1}`, "junit.xml"), xml([["scenarios::steady", "pass"], ["scenarios::odd", v]]));
    }
    const found = flaky(dir, { source: "stress-x20" });
    assert.equal(found.differences.length, 1);
    assert.equal(found.issues[0].title, "Flaky scenario: purlis-app scenarios::odd");
    assert.match(found.issues[0].body, /stress-x20/);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test("above_five_differing_entries_one_summary_issue_lists_them_and_an_escape_still_opens_its_own", () => {
  const many = (n) => Array.from({ length: n }, (_, i) => rec(`M-NET-${i}`, "pass"));
  const flip = (records) => records.map((r) => ({ ...r, verdict: "fail" }));
  const dir = runsOf([many(6), flip(many(6)), many(6)]);
  try {
    const { issues } = flaky(dir, { source: "nightly" });
    assert.equal(issues.length, 6);
    const out = capped(issues, { source: "nightly" });
    assert.equal(out.length, 1);
    assert.match(out[0].title, /^Flaky scenarios: many entries differ/);
    for (let i = 0; i < 6; i += 1) assert.match(out[0].body, new RegExp(`M-NET-${i}\\b.*run 1: pass; run 2: fail; run 3: pass`));
    // Five or fewer open one each; an escape is never folded in.
    assert.equal(capped(issues.slice(0, 5)).length, 5);
    const escape = { title: "Escape verdict differs between runs: x", labels: ["area:security", "priority:p0", "bot-maintained"], body: "b" };
    assert.equal(capped([...issues, escape]).length, 2);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test("an_issue_is_found_by_its_exact_title_past_a_quote_and_the_first_page_and_its_label_is_made", () => {
  const issue = { title: 'Flaky scenario: a "quoted" row', labels: ["flaky-scenario", "bot-maintained"], body: "b" };
  const calls = [];
  const said = [];
  // No such issue open, and no such label: both are created.
  open(issue, {
    repo: "o/r",
    say: (m) => said.push(m),
    run: (...args) => (calls.push(args), args[0] === "issue" && args[1] === "list" ? "[]" : ""),
  });
  const list = calls.find((c) => c[1] === "list");
  assert.equal(list[list.indexOf("--search") + 1], '"Flaky scenario: a \\"quoted\\" row" in:title');
  assert.equal(list[list.indexOf("--limit") + 1], "1000");
  assert.ok(calls.some((c) => c[0] === "label" && c[1] === "create" && c[2] === "flaky-scenario"));
  assert.ok(calls.some((c) => c[0] === "issue" && c[1] === "create"));
  // One open with that title is commented on, and nothing is created.
  const again = [];
  open(issue, {
    repo: "o/r",
    say: (m) => said.push(m),
    run: (...args) => (again.push(args), args[1] === "list" ? JSON.stringify([{ number: 7, title: issue.title }]) : ""),
  });
  assert.ok(again.some((c) => c[1] === "comment" && c[2] === "7"));
  assert.ok(!again.some((c) => c[1] === "create" || c[0] === "label"));
});

test("an_issue_that_could_not_be_opened_is_an_error_and_the_run_stays_red_without_a_dry_run", () => {
  const dir = runsOf([[rec("M-NET-01", "pass")], [rec("M-NET-01", "fail")], [rec("M-NET-01", "pass")]]);
  try {
    // `--open` with no gh that works (none on this PATH): the failure is an error, never a warning.
    const ran = spawnSync(process.execPath, [TOOL, "flaky", dir, "--open", "--cells", "/nonexistent"], {
      encoding: "utf8",
      env: { ...process.env, PATH: "/nonexistent", GITHUB_REPOSITORY: "o/r" },
    });
    assert.equal(ran.status, 1);
    assert.match(ran.stdout, /::error title=flaky scenario::could not open/);
    assert.doesNotMatch(ran.stdout, /::warning/);
    // Without `--open` the runs are compared and nothing is opened or attempted.
    const compared = spawnSync(process.execPath, [TOOL, "flaky", dir, "--cells", "/nonexistent"], { encoding: "utf8", env: { ...process.env, PATH: "/nonexistent" } });
    assert.equal(compared.status, 1);
    assert.doesNotMatch(compared.stdout, /could not open/);
  } finally {
    rmSync(dir, { recursive: true });
  }
});
