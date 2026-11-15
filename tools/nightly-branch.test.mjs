// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Main's scheduler for an integration branch's nightlies (`nightly-branch.yml`, M74.1 T1.1-09):
// GitHub runs `schedule` only from the default branch, so it lives on main, checks the branch out,
// and runs each entry of the branch's list through the branch's `.github/actions/nightly`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { jobsOf } from "./ci-push-jobs.mjs";

const text = readFileSync(new URL("../.github/workflows/nightly-branch.yml", import.meta.url), "utf8");
const code = text
  .split("\n")
  .filter((line) => !/^\s*#/.test(line))
  .join("\n");
const job = (id) => {
  const found = jobsOf(text).find((j) => j.id === id);
  assert.ok(found, `nightly-branch.yml has no job ${id}`);
  return found;
};
const steps = (j) => j.text.split(/\n(?= {6}- )/).slice(1);

test("the_main_scheduler_runs_a_listed_branch_entry", () => {
  assert.match(code, /^ {2}schedule:\n {4}- cron: "[^"]+"/m);
  assert.match(code, /^ {2}workflow_dispatch:/m);
  // The branch is one variable: the checkouts and the list's location all read it.
  const branches = [...code.matchAll(/^ {2}BRANCH: (\S+)$/gm)];
  assert.equal(branches.length, 1);
  assert.equal(branches[0][1], "phase-1-proven");
  assert.doesNotMatch(code.replace(/^ {2}BRANCH: .*$/m, ""), /phase-1-proven/);

  const list = job("list");
  assert.match(list.text, /ref: \$\{\{ env\.BRANCH \}\}/);
  assert.match(list.text, /node tools\/nightly-list\.mjs "\.github\/nightly-on-\$BRANCH\.txt"/);

  const entry = job("entry");
  assert.deepEqual(entry.needs, ["list"]);
  assert.match(entry.text, /matrix: \$\{\{ fromJSON\(needs\.list\.outputs\.matrix\) \}\}/);
  assert.match(entry.text, /runs-on: \$\{\{ matrix\.runs-on \}\}/);
  assert.match(entry.text, /timeout-minutes: \$\{\{ matrix\.timeout \}\}/);
  assert.match(entry.text, /ref: \$\{\{ env\.BRANCH \}\}/);
  assert.match(entry.text, /uses: \.\/\.github\/actions\/nightly\n\s+with:\n\s+entry: \$\{\{ matrix\.entry \}\}\n\s+quiet: \$\{\{ matrix\.quiet \}\}/);
  // Issues are opened by this workflow's token.
  assert.match(code, /^permissions:\n {2}contents: read\n {2}issues: write$/m);
});

test("a_red_escape_entry_on_the_public_repo_opens_no_issue_and_uploads_no_artifacts", () => {
  const entry = job("entry");
  const reporting = steps(entry).filter((s) => /upload-artifact|gh issue/.test(s));
  assert.ok(reporting.some((s) => /upload-artifact/.test(s)), "a loud red night uploads its artifacts");
  assert.ok(reporting.some((s) => /gh issue create/.test(s)), "a loud red night opens an issue");
  for (const step of reporting) assert.match(step, /if: failure\(\) && !matrix\.quiet\n/, step);
  // A quiet entry's red says its id and nothing more.
  const quiet = steps(entry).filter((s) => /if: failure\(\) && matrix\.quiet\n/.test(s));
  assert.equal(quiet.length, 1);
  assert.doesNotMatch(quiet[0], /upload-artifact|gh issue|cat |\.log/);
  assert.match(quiet[0], /matrix\.entry/);
});
