#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The nightly list** (M74.1, T1.1-09): what main's scheduler (`nightly-branch.yml` on main) runs
// each night against an integration branch. GitHub runs `schedule` only from the default branch, so
// the scheduler lives on main, checks out the branch, and turns the branch's
// `.github/nightly-on-<branch>.txt` into its matrix with this tool. Each entry then runs as
// `uses: ./.github/actions/nightly` with `entry:`, on its own runner, from the branch's checkout.
//
// One entry a line; `#` starts a comment, and a blank line is nothing:
//
//     <entry>  <runs-on>  <timeout-minutes>  [repo: mirror]  [kind: escape]
//
// - `entry`: a name `.github/actions/nightly` knows, lowercase words and dashes.
// - `runs-on`: the runner label. An entry may be listed once per runner.
// - `timeout-minutes`: the job's limit, 1 to 360.
// - `repo: mirror`: runs only when the workflow's repository is the private mirror (ticket 57),
//   which the scheduler names in its `NIGHTLY_MIRROR` repository variable. Unset, no repository is
//   the mirror, and these entries never run.
// - `kind: escape`: an entry whose red may say how the sandbox was escaped (an escape cell, a
//   refused-call recording that changed). On any repository but the mirror it runs quiet: its log
//   prints only its id, and it opens no issue and uploads no artifacts (Q287, Q298).
//
// `node tools/nightly-list.mjs <list> [repository] [mirror]` writes `matrix` (JSON, for
// `strategy.matrix: fromJSON(...)`) and `count` to `$GITHUB_OUTPUT` (or stdout). A malformed list
// fails, naming its line: a scheduler that quietly runs less is worse than a red night.

import { appendFileSync, readFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

const OPTIONAL = { repo: ["mirror"], kind: ["escape"] };

/** The list's entries: `{ entry, runsOn, timeout, repo, kind }`, in order. */
export function parseList(text) {
  const entries = [];
  const seen = new Set();
  text.split("\n").forEach((raw, index) => {
    const line = raw.replace(/#.*/, "").trim();
    if (line === "") return;
    const where = `line ${index + 1}`;
    const [entry, runsOn, timeoutText, ...rest] = line.split(/\s+/);
    if (timeoutText === undefined) throw new Error(`${where}: an entry needs its runs-on and timeout`);
    if (!/^[a-z0-9][a-z0-9-]*$/.test(entry)) throw new Error(`${where}: "${entry}" is not an entry name`);
    const timeout = Number(timeoutText);
    if (!Number.isInteger(timeout) || timeout < 1 || timeout > 360) {
      throw new Error(`${where}: the timeout "${timeoutText}" is not a whole number of minutes from 1 to 360`);
    }
    const fields = { repo: undefined, kind: undefined };
    const words = rest.join(" ").split(/\s*:\s*|\s+/).filter(Boolean);
    for (let i = 0; i < words.length; i += 2) {
      const [key, value] = [words[i], words[i + 1]];
      if (!(key in OPTIONAL)) throw new Error(`${where}: "${key}" is not a field (repo, kind)`);
      if (!OPTIONAL[key].includes(value)) {
        throw new Error(`${where}: ${key} is ${OPTIONAL[key].join(" or ")}, not "${value}"`);
      }
      fields[key] = value;
    }
    const id = `${entry} ${runsOn}`;
    if (seen.has(id)) throw new Error(`${where}: ${entry} is listed twice on ${runsOn}`);
    seen.add(id);
    entries.push({ entry, runsOn, timeout, ...fields });
  });
  return entries;
}

/**
 * The scheduler's matrix for a repository: `{ matrix: { include }, skipped }`. A `repo: mirror`
 * entry is skipped unless `repository` is the named `mirror`; a `kind: escape` entry runs quiet
 * anywhere but the mirror.
 */
export function matrixOf(entries, { repository, mirror }) {
  const onMirror = Boolean(mirror) && repository === mirror;
  const include = [];
  const skipped = [];
  for (const e of entries) {
    if (e.repo === "mirror" && !onMirror) {
      skipped.push(e.entry);
      continue;
    }
    include.push({ entry: e.entry, "runs-on": e.runsOn, timeout: e.timeout, quiet: e.kind === "escape" && !onMirror });
  }
  return { matrix: { include }, skipped };
}

function main([list, repository = "", mirror = ""]) {
  const out = process.env.GITHUB_OUTPUT;
  const say = (line) => (out ? appendFileSync(out, `${line}\n`) : console.log(line));
  const { matrix, skipped } = matrixOf(parseList(readFileSync(list, "utf8")), { repository, mirror });
  for (const entry of skipped) console.log(`::notice::${entry} runs on the mirror only; skipped here`);
  say(`matrix=${JSON.stringify(matrix)}`);
  say(`count=${matrix.include.length}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
