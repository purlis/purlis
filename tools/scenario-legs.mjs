#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **What the sandbox scenario legs' reports say across runs** (M74.1, T1.1-54, #6901).
//
// Each leg writes one JSON record a line (`tests/scenarios/schema/leg-report.schema.json`). The
// nightly runs the suite three times in a shuffled order of its batches, and `stress-x20` runs a
// pull request's changed batches twenty times: this tool compares the runs' verdicts, shuffles,
// and says which batches a pull request changed.
//
//     node tools/scenario-legs.mjs flaky <runs-dir> [--source nightly|stress-x20] [--dry-run]
//     node tools/scenario-legs.mjs shuffle <seed>          # test names on stdin, groups on stdout
//     node tools/scenario-legs.mjs changed-batches <base>  # a nextest filter for the PR's batches
//     STEPS='<toJSON(steps)>' node tools/scenario-legs.mjs steps <row> <os>   # steps -> records
//
// - **flaky**: `<runs-dir>/<run>/**.jsonl` is one run each. An entry whose verdict differs between
//   runs is flaky, and counts as failing until fixed: the command exits 1. Each opens (or comments
//   on) a public issue, labelled `flaky-scenario` and `bot-maintained`. An entry that is an escape
//   (`kind: escape`, or a cell with `escape: true` or a step that expects a refusal, so a refusal
//   that did not hold) opens one labelled `area:security` and `priority:p0` instead: everything
//   runs in public (Q301), and nothing goes anywhere private. `--dry-run` prints each issue it
//   would open, as JSON, and opens none; so does `FLAKY_DRY_RUN=1`. Each run's `junit.xml`
//   (nextest's) is compared with its leg report, test by test. `GH_TOKEN` and `GITHUB_REPOSITORY` name who opens them.
// - **shuffle**: the batches of a suite, in an order seeded by `<seed>`. A batch is a test's module
//   path (everything before its last `::`), so the order of batches changes between runs and the
//   order of a batch's own cells never does (cells depend on earlier cells' state).
// - **changed-batches**: a nextest filter that picks the tests of each `tests/scenarios/cells/<B>.json`
//   changed since `<base>`: a test belongs to batch B when `b` is one of its module path's
//   segments, in any case. With none changed it prints nothing, and the caller runs the whole suite.

import { execFileSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** The records of every `.jsonl` file under `path` (a file or a folder), in file-name order. */
export function readReports(path) {
  if (!existsSync(path)) return [];
  if (statSync(path).isFile()) {
    return readFileSync(path, "utf8")
      .split("\n")
      .filter((line) => line.trim() !== "")
      .map((line, index) => {
        try {
          return JSON.parse(line);
        } catch (error) {
          throw new Error(`${path}:${index + 1}: not JSON: ${error.message}`);
        }
      })
      .filter((record) => record.kind !== undefined && record.verdict !== undefined);
  }
  return readdirSync(path)
    .sort()
    .flatMap((name) => {
      const inner = join(path, name);
      return statSync(inner).isDirectory() || name.endsWith(".jsonl") ? readReports(inner) : [];
    });
}

/**
 * The verdicts of the tests in nextest's JUnit file `xml`, as records of kind `test`: `row` is the
 * test's full name, and the verdict is `pass`, `fail` or `not_run` (skipped). A run's tests are
 * compared beside its leg report, so a scenario that passes in one run and fails in another is
 * found whether or not the cell it plays writes a record of its own.
 */
export function junitRecords(xml) {
  const out = [];
  const cases = xml.split("<testcase ").slice(1);
  for (const chunk of cases) {
    const head = chunk.slice(0, chunk.indexOf(">"));
    const attr = (name) => new RegExp(`\\b${name}="([^"]*)"`).exec(head)?.[1];
    const body = head.endsWith("/") ? "" : chunk.slice(chunk.indexOf(">"), chunk.indexOf("</testcase>"));
    const name = attr("name");
    if (!name) continue;
    const verdict = /<(failure|error|flakyFailure|flakyError|rerunFailure)\b/.test(body) ? "fail" : /<skipped\b/.test(body) ? "not_run" : "pass";
    out.push({ kind: "test", row: `${attr("classname") ?? ""} ${name}`.trim(), os: "any", leg: "junit", verdict });
  }
  return out;
}

/** What names one entry of a leg's report, across runs. */
export const keyOf = (r) =>
  [r.kind, r.row, r.test ?? "", r.dialect ?? "", r.protection ?? "", r.os, r.leg, r.timings ?? "scaled"].join("|");

const describe = (r) =>
  r.kind === "test"
    ? r.row
    : [r.row, r.test, r.dialect, r.protection, r.os, `${r.leg} leg`, r.timings ? `${r.timings} timings` : undefined]
        .filter(Boolean)
        .join(", ");

/**
 * The cell ids that are escapes in the cells under `folder`: a cell that says `"escape": true`,
 * and one with a step that expects a refusal (`refusal_expected`), so a refusal that did not hold
 * in one run and held in another is read as an escape and never as noise.
 */
export function escapeRows(folder) {
  const ids = new Set();
  if (!existsSync(folder)) return ids;
  for (const name of readdirSync(folder)) {
    if (!name.endsWith(".json")) continue;
    const batch = JSON.parse(readFileSync(join(folder, name), "utf8"));
    for (const cell of batch.cells ?? []) {
      if (cell.escape === true || (cell.steps ?? []).some((step) => step.refusal_expected === true)) ids.add(cell.id);
    }
  }
  return ids;
}

/**
 * **The entries whose verdict differs between `runs`** (one array of records each). An entry a run
 * has no record of counts as `missing` there, so a run that did not pick a cell up differs from one
 * that did. `escapes` is the set of row ids that are escapes.
 * Returns `[{ key, what, escape, verdicts }]`, in the order the entries first appear.
 */
export function differences(runs, escapes = new Set()) {
  const seen = new Map();
  runs.forEach((records, index) => {
    for (const r of records) {
      const key = keyOf(r);
      if (!seen.has(key)) seen.set(key, { sample: r, verdicts: new Array(runs.length).fill("missing") });
      const entry = seen.get(key);
      // A pass anywhere in the run is the run's verdict, as `judge_runs` reads it.
      if (entry.verdicts[index] !== "pass") entry.verdicts[index] = r.verdict;
    }
  });
  const out = [];
  for (const [key, { sample, verdicts }] of seen) {
    if (new Set(verdicts).size < 2) continue;
    out.push({
      key,
      what: describe(sample),
      escape: sample.kind === "escape" || escapes.has(sample.row),
      verdicts,
    });
  }
  return out;
}

/** The issue one differing entry opens, in public (Q301). */
export function issueOf(difference, { source, runUrl = "" }) {
  const where = source === "stress-x20" ? "under CPU stress (stress-x20)" : "in the nightly's three shuffled runs";
  const verdicts = difference.verdicts.map((v, i) => `run ${i + 1}: ${v}`).join("; ");
  if (difference.escape) {
    return {
      title: `Escape verdict differs between runs: ${difference.what}`,
      labels: ["area:security", "priority:p0", "bot-maintained"],
      body: `An escape entry, or a refusal, had a different verdict ${where}: ${verdicts}.\n\nA refusal that held in one run and not in another is an escape until shown otherwise: fix this first. ${runUrl}`.trim(),
    };
  }
  return {
    title: `Flaky scenario: ${difference.what}`,
    labels: ["flaky-scenario", "bot-maintained"],
    body: `A scenario's verdict differed ${where}: ${verdicts}.\n\nIt counts as failing until it is fixed (spec principle 5: no flake); nothing retries it. ${runUrl}`.trim(),
  };
}

/**
 * **The flaky entries of the runs under `dir`**, and the issues they open: `{ differences, issues }`.
 * Each folder under `dir` is one run.
 */
export function flaky(dir, { cells, source = "nightly", runUrl = "" } = {}) {
  const runs = readdirSync(dir)
    .sort()
    .filter((name) => statSync(join(dir, name)).isDirectory())
    .map((name) => {
      // nextest's JUnit file of each of the run's calls (one a batch): `junit.xml`, `junit-<k>.xml`.
      const junit = readdirSync(join(dir, name))
        .filter((f) => /^junit(-[^/]*)?\.xml$/.test(f))
        .sort()
        .flatMap((f) => junitRecords(readFileSync(join(dir, name, f), "utf8")));
      return [...readReports(join(dir, name)), ...junit];
    });
  const found = differences(runs, cells ? escapeRows(cells) : new Set());
  return { runs: runs.length, differences: found, issues: found.map((d) => issueOf(d, { source, runUrl })) };
}

// ---- shuffle -------------------------------------------------------------------------------

/** A small seeded generator (mulberry32): the same seed, the same order. */
export function random(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The batch a test belongs to: its module path, everything before the last `::`. */
export const batchOf = (name) => (name.includes("::") ? name.slice(0, name.lastIndexOf("::")) : name);

/** The batches of `names`, once each, in the order `seed` gives. A batch's own tests are not moved. */
export function shuffled(names, seed) {
  const batches = [...new Set(names.map(batchOf))].sort();
  const next = random(seed);
  for (let i = batches.length - 1; i > 0; i -= 1) {
    const j = Math.floor(next() * (i + 1));
    [batches[i], batches[j]] = [batches[j], batches[i]];
  }
  return batches;
}

// ---- changed batches -----------------------------------------------------------------------

/** The batch ids whose cells file is among `paths`. */
export function batchesChanged(paths) {
  return paths
    .map((path) => /^tests\/scenarios\/cells\/([A-Za-z0-9_-]+)\.json$/.exec(path)?.[1])
    .filter(Boolean)
    .sort();
}

/** A nextest filter for the tests of `batches`, or `""` for none. */
export function filterOf(batches) {
  if (batches.length === 0) return "";
  const words = batches.map((b) => b.toLowerCase().replace(/[^a-z0-9_]/g, "_"));
  return `test(/(^|::)(${words.join("|")})::/)`;
}

// ---- a composite action's steps as records --------------------------------------------------------

/**
 * **The leg-report records of the steps of a job** (`toJSON(steps)`): one `fixture` record for
 * each step that has an id, named for it, with the step's outcome as its verdict. A step that was
 * skipped is `not_run` with its reason, never a pass. Used by the existing macOS sandbox tests,
 * whose targets are steps of one action (T1.1-54, item 10).
 */
export function recordsOfSteps(steps, row, os) {
  return Object.entries(steps).map(([id, step]) => {
    const base = { kind: "fixture", row, test: id, os, leg: "fixture", timings: "scaled" };
    if (step.outcome === "success") return { ...base, verdict: "pass" };
    if (step.outcome === "skipped") return { ...base, verdict: "not_run", reason: "the step was skipped" };
    return { ...base, verdict: "fail" };
  });
}

// ---- command line --------------------------------------------------------------------------

function open(issue, { repo, say }) {
  const run = (...args) => execFileSync("gh", args, { encoding: "utf8" });
  const found = JSON.parse(
    run("issue", "list", "-R", repo, "--state", "open", "--label", "bot-maintained", "--search", `"${issue.title}" in:title`, "--json", "number,title"),
  ).find((i) => i.title === issue.title);
  if (found) {
    run("issue", "comment", String(found.number), "-R", repo, "--body", `Differs again: ${issue.body}`);
    say(`commented on #${found.number}: ${issue.title}`);
  } else {
    run("issue", "create", "-R", repo, "--title", issue.title, "--label", issue.labels.join(","), "--body", issue.body);
    say(`opened: ${issue.title}`);
  }
}

function main([command, ...rest]) {
  const flag = (name) => {
    const at = rest.indexOf(name);
    return at === -1 ? undefined : rest[at + 1];
  };
  if (command === "flaky") {
    const dir = rest.find((a) => !a.startsWith("--") && a !== flag("--source") && a !== flag("--cells"));
    if (!dir || !existsSync(dir)) {
      console.error(`usage: scenario-legs.mjs flaky <runs-dir> [--source nightly|stress-x20] [--dry-run]; ${dir} is not a folder`);
      return 2;
    }
    const dry = rest.includes("--dry-run") || process.env.FLAKY_DRY_RUN === "1";
    const runUrl = process.env.GITHUB_SERVER_URL
      ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}`
      : "";
    const result = flaky(dir, { cells: flag("--cells") ?? fileURLToPath(new URL("../tests/scenarios/cells", import.meta.url)), source: flag("--source"), runUrl });
    console.log(`${result.runs} runs compared: ${result.differences.length} entries differ`);
    for (const issue of result.issues) {
      if (dry) console.log(JSON.stringify({ wouldOpen: issue }));
      else {
        // A token that cannot open an issue (a fork's pull request) must not hide the red.
        try {
          open(issue, { repo: process.env.GITHUB_REPOSITORY, say: console.log });
        } catch (error) {
          console.log(`::warning title=flaky scenario::could not open "${issue.title}": ${error.message.split("\n")[0]}`);
        }
      }
    }
    for (const d of result.differences) console.log(`::error title=${d.escape ? "escape verdict differs" : "flaky scenario"}::${d.what}: ${d.verdicts.join(", ")}`);
    return result.differences.length === 0 ? 0 : 1;
  }
  if (command === "shuffle") {
    const seed = Number(rest[0]);
    if (!Number.isInteger(seed)) {
      console.error("usage: scenario-legs.mjs shuffle <seed>, test names on stdin");
      return 2;
    }
    const names = readFileSync(0, "utf8").split("\n").map((l) => l.trim()).filter(Boolean);
    for (const batch of shuffled(names, seed)) console.log(batch);
    return 0;
  }
  if (command === "steps") {
    const [row, os] = rest;
    const records = recordsOfSteps(JSON.parse(process.env.STEPS ?? "{}"), row, os);
    const file = join("target", "sandbox-scenarios", `${os}-${row}.jsonl`);
    mkdirSync(dirname(file), { recursive: true });
    appendFileSync(file, records.map((r) => `${JSON.stringify(r)}\n`).join(""));
    console.log(`${records.length} records in ${file}: ${records.filter((r) => r.verdict !== "pass").length} not green`);
    return 0;
  }
  if (command === "changed-batches") {
    const paths = execFileSync("git", ["diff", "--name-only", "--no-renames", rest[0], "HEAD"], { encoding: "utf8" }).split("\n").filter(Boolean);
    const filter = filterOf(batchesChanged(paths));
    if (filter) console.log(filter);
    return 0;
  }
  console.error("usage: scenario-legs.mjs flaky|shuffle|changed-batches");
  return 2;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
