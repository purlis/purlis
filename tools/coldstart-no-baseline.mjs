#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// CI's step, when main's app did not build in a pull request's job (#2388):
//
//   node tools/coldstart-no-baseline.mjs <main's sha>
//
// It reads main's own `app builds (ubuntu-24.04)` check on that commit (`gh api`, with the job's
// token) and lets `withoutMainsBuild` decide. main not green there either: the reason goes to the
// step's output as `no-baseline`, for the cold start to report and gate nothing, and the step
// passes. main green there, or its check unreadable: an error annotation, and the step fails, so
// a change that broke main's build only in its own job cannot switch its own gate off.
import { spawnSync } from "node:child_process";
import { appendFileSync } from "node:fs";

import { MAINS_BUILD_CHECK, withoutMainsBuild } from "./coldstart-gate.mjs";

const [sha] = process.argv.slice(2);
if (!sha) {
  console.error("coldstart-no-baseline.mjs: which commit is main's");
  process.exit(2);
}

const read = spawnSync(
  "gh",
  [
    "api",
    "-X",
    "GET",
    `repos/${process.env.GITHUB_REPOSITORY}/commits/${sha}/check-runs`,
    "-f",
    `check_name=${MAINS_BUILD_CHECK}`,
  ],
  { encoding: "utf8" },
);
let found;
if (read.status === 0) {
  try {
    found = { runs: JSON.parse(read.stdout).check_runs ?? [] };
  } catch (error) {
    found = { error: `gh api said what is not JSON: ${error.message}` };
  }
} else {
  found = { error: (read.stderr || read.error?.message || `gh exited ${read.status}`).trim() };
}

const decided = withoutMainsBuild({ sha, ...found });
console.log(decided.annotation);
if (decided.fails) process.exit(1);
appendFileSync(process.env.GITHUB_OUTPUT, `no-baseline=${decided.noBaseline}\n`);
