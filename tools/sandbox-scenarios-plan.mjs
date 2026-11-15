#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What a run of `.github/workflows/sandbox-scenarios.yml` runs (M74.1, T1.1-09): the Linux suite,
// the macOS legs, the production-timing entries and the stress run, from the run's event, its
// pull request's labels and the paths it changes.
//
// - **The Linux job always runs** and skips inside, never by a path filter: a path-filtered
//   required check hangs or is bypassed. The skip is a deny-list: only a pull request that
//   changes documentation alone skips (`documentationAlone`). A lockfile, the toolchain, the
//   plugin or a workflow is never documentation, and nothing skips but a pull request.
// - **macOS** runs in the merge queue and on a pull request labelled `macos-scenarios`.
// - **`production-timings`** runs the production-timing entries (ticket 54) on the pull request's
//   head, on Linux, and on macOS when it has both labels. The merge queue runs them always.
// - **`stress-x20`** runs the changed batches 20 times under a CPU stress (ticket 54 fills it).
//
// Run as `node tools/sandbox-scenarios-plan.mjs <event> <base>` in a checkout whose HEAD is the
// change, with the pull request's labels as JSON in `LABELS`: it writes `linux`, `macos`,
// `production-timings`, `stress` and `why` to `$GITHUB_OUTPUT` (or stdout). A change it cannot
// read runs everything.

import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { AGENT_STEERING, scopesOfPath } from "./ci-scope.mjs";

export const LABELS = {
  macos: "macos-scenarios",
  productionTimings: "production-timings",
  stress: "stress-x20",
};

// `ci-scope.mjs`'s answer for a doc nothing reads: the docs site and the tools' tests.
const DOCS_ONLY = ["docs", "tools"];

/**
 * Whether a changed path is documentation and nothing else: under `docs/`, `site/` or `changes/`,
 * or an agent's steering, and read by no code (`ci-scope.mjs`'s `READ_BY_CODE` and
 * `DOCS_READ_BY_RUST`). A deleted doc is not: a file a test holds may link to it.
 */
export function documentationAlone(path, deleted = new Set()) {
  if (deleted.has(path)) return false;
  if (!/^(docs|site|changes)\//.test(path) && !AGENT_STEERING.has(path)) return false;
  const scopes = [...scopesOfPath(path, new Set())].sort();
  return scopes.length === DOCS_ONLY.length && scopes.every((s, i) => s === DOCS_ONLY[i]);
}

/** The plan for one run: `{ linux, macos, productionTimings, stress, why }`. */
export function plan({ event, paths = [], labels = [], deleted = [] }) {
  const has = (label) => labels.includes(label);
  if (event !== "pull_request") {
    return {
      linux: true,
      macos: true,
      productionTimings: event === "merge_group",
      stress: false,
      why: `a ${event} run never skips: the suite runs on every leg`,
    };
  }
  const gone = new Set(deleted);
  if (paths.length > 0 && paths.every((path) => documentationAlone(path, gone))) {
    return {
      linux: false,
      macos: false,
      productionTimings: false,
      stress: false,
      why: `this pull request changes documentation alone (${paths.join(", ")}), so the suite is skipped`,
    };
  }
  const macos = has(LABELS.macos);
  const legs = ["Linux", ...(macos ? ["macOS"] : [])].join(" and ");
  return {
    linux: true,
    macos,
    productionTimings: has(LABELS.productionTimings),
    stress: has(LABELS.stress),
    why: `the suite runs on ${legs}: this pull request changes more than documentation`,
  };
}

function main([event, base]) {
  const out = process.env.GITHUB_OUTPUT;
  const say = (line) => (out ? appendFileSync(out, `${line}\n`) : console.log(line));
  let result;
  try {
    const labels = JSON.parse(process.env.LABELS || "[]") ?? [];
    const paths = [];
    const deleted = [];
    if (event === "pull_request") {
      const lines = execFileSync("git", ["diff", "--name-status", "--no-renames", base, "HEAD"], {
        encoding: "utf8",
        maxBuffer: 1 << 28,
      }).split("\n");
      for (const line of lines) {
        const [status, path] = line.split("\t");
        if (!path) continue;
        paths.push(path);
        if (status === "D") deleted.push(path);
      }
    }
    result = plan({ event, paths, labels, deleted });
  } catch (error) {
    result = { ...plan({ event: "unreadable" }), why: `the change could not be read, so everything runs: ${error.message}` };
  }
  console.log(`::notice title=sandbox scenarios::${result.why}`);
  say(`linux=${result.linux}`);
  say(`macos=${result.macos}`);
  say(`production-timings=${result.productionTimings}`);
  say(`stress=${result.stress}`);
  say(`why=${result.why.replace(/\n/g, " ")}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
