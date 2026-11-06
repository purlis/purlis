#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The nightly scale run's report (#5497, spec #5486): each budget of the spec's targets table
// against what the run measured on macOS and Linux, written to the job summary, and kept in one
// issue labelled `scale-regression`, the way `ci-health.mjs` keeps its one issue.
//
// A miss opens the issue (or reopens it), with the timings and the commits since the last pass;
// a later miss updates it; a pass closes it. A run that measured too little to say either way
// touches no issue. The last pass is kept in the issue's own body, so nothing else is stored.
//
//   node tools/scale-report.mjs <rows.jsonl>…                 # the summary, to $GITHUB_STEP_SUMMARY or stdout
//   GITHUB_TOKEN=… node tools/scale-report.mjs --issue <rows.jsonl>…   # and the one issue
//   node tools/scale-report.mjs --only linux <rows.jsonl>     # one system's, for its own job
//
// A row is one JSON object a line: `{"budget": "hook", "os": "linux", "samples": [ms, …]}`, or
// `{"budget": "idle-cpu", "os": "macos", "value": 1.5}` where the measurement is one number.
// `tools/scale-run.mjs` writes them. Everything above the I/O below is pure and tested in
// `tools/scale-report.test.mjs`.

import { readFileSync, appendFileSync } from "node:fs";
import process from "node:process";

/** The systems the run measures on, by the word a row names them with. */
export const SYSTEMS = { macos: "macOS", linux: "Linux" };

/**
 * Every row the report holds, in its order. `limit` is the budget in `unit`, `strict` a budget
 * that must be stayed under rather than reached, `only` the system a budget is held on (the
 * other reports against none), and `pending` the ticket whose work it waits for: until then it
 * is not measured, and not counted as missing. A row with no `limit` is reported only. The
 * budgets are #5486's targets table; the rest are the measurements #5497 asks for beside them.
 */
export const BUDGETS = [
  { budget: "hook", what: "SessionStart hook", stat: "p95", limit: 50, unit: "ms" },
  { budget: "recall", what: "`recall`, in one workspace", stat: "p95", limit: 100, unit: "ms" },
  { budget: "recall-all", what: "`recall --all-workspaces`", stat: "p95", limit: 100, unit: "ms" },
  { budget: "project-search", what: "Project search", stat: "p95", limit: 100, strict: true, unit: "ms", pending: "#5498" },
  { budget: "index-rebuild", what: "Full index rebuild", stat: "max", limit: 40_000, unit: "ms", pending: "#5498" },
  { budget: "watcher-setup", what: "Watcher setup", stat: "max", limit: 1000, unit: "ms", only: "macos" },
  { budget: "sidebar", what: "Sidebar answer", stat: "p95", unit: "ms" },
  { budget: "git-standing", what: "One git standing look", stat: "p50", unit: "ms" },
  { budget: "idle-cpu", what: "Idle CPU, a minute with a window shown", stat: "mean", unit: "%" },
  { budget: "chat-events", what: "Chat events reaching the window, batched", stat: "mean", unit: "/s" },
  { budget: "chat-events-unbatched", what: "Chat events reaching the window, unbatched", stat: "mean", unit: "/s" },
];

/** The `p`th percentile of `samples` by nearest rank. */
export function percentile(samples, p) {
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil((p / 100) * sorted.length) - 1)];
}

function statOf(row, stat) {
  if (typeof row.value === "number") return row.value;
  const samples = row.samples ?? [];
  if (samples.length === 0) return undefined;
  if (stat === "max") return Math.max(...samples);
  if (stat === "mean") return samples.reduce((a, b) => a + b, 0) / samples.length;
  return percentile(samples, Number(stat.slice(1)));
}

/**
 * One verdict for each budget on each system: `met`, `missed`, `reported` (no budget there),
 * `pending` (its subject is not built yet) or `not measured` (it should have been and was not).
 * `rows` are what the run measured; a reported-only measurement nobody took is left out.
 * `only` names the one system a report is of, as a measuring job's own summary is.
 */
export function verdicts(rows, { only } = {}) {
  const found = [];
  for (const spec of BUDGETS) {
    for (const os of Object.keys(SYSTEMS).filter((os) => only === undefined || os === only)) {
      const base = { budget: spec.budget, os, spec };
      if (spec.pending) {
        found.push({ ...base, status: "pending" });
        continue;
      }
      const row = rows.find((r) => r.budget === spec.budget && r.os === os);
      const value = row ? statOf(row, spec.stat) : undefined;
      const held = spec.limit !== undefined && (spec.only === undefined || spec.only === os);
      if (value === undefined) {
        if (held) found.push({ ...base, status: "not measured" });
        continue;
      }
      const status = !held ? "reported" : (spec.strict ? value < spec.limit : value <= spec.limit) ? "met" : "missed";
      found.push({ ...base, value, samples: row.samples?.length, status });
    }
  }
  return found;
}

/**
 * What the run does to the one issue (`issue`: `{number, state}`, or undefined when there is
 * none yet): `open` it (reopening `number` when it exists) on any miss, `update` it on a miss
 * while it is open, `close` it once every budget was measured and met, and `none` otherwise.
 */
export function decide(found, issue) {
  if (found.some((v) => v.status === "missed")) {
    if (issue?.state === "open") return { action: "update", number: issue.number };
    return issue ? { action: "open", number: issue.number } : { action: "open" };
  }
  const passed = !found.some((v) => v.status === "not measured");
  if (passed && issue?.state === "open") return { action: "close", number: issue.number };
  return { action: "none" };
}

const number = (value) => (Number.isInteger(value) ? String(value) : value.toFixed(value < 10 ? 2 : 0));

function shown(v) {
  if (v.value === undefined) return "—";
  const unit = v.spec.unit === "ms" || v.spec.unit === "/s" ? ` ${v.spec.unit}`.replace(" /s", "/s") : v.spec.unit;
  return `${v.spec.stat} ${number(v.value)}${unit}`;
}

function budgetOf(v) {
  const { limit, strict, unit, only } = v.spec;
  if (limit === undefined || (only !== undefined && only !== v.os)) return "none";
  const value = unit === "ms" && limit >= 1000 ? `${limit / 1000} s` : `${limit} ${unit}`;
  return `${strict ? "<" : "≤"} ${value}`;
}

const statusOf = (v) => (v.status === "pending" ? `pending (${v.spec.pending})` : v.status);

/** The report as Markdown: the job summary, and the table in the issue's body. */
export function summary(found, { head, run } = {}) {
  const missed = found.filter((v) => v.status === "missed").length;
  const lines = [
    `## Scale run${head ? ` at ${head.slice(0, 7)}` : ""}`,
    "",
    "Evidence only (#5497): 1,000 workspaces, 100 personas, 100,000 + 100,000 memories, on a project " +
      "`scale-project` generated from its seed. A budget is #5486's target; the rest is reported.",
    "",
    missed > 0 ? `**${missed} budget${missed === 1 ? "" : "s"} missed.**` : "**No budget missed.**",
    "",
    "| What | System | Measured | Budget | Status |",
    "| --- | --- | --- | --- | --- |",
    ...found.map((v) => `| ${v.spec.what} | ${SYSTEMS[v.os]} | ${shown(v)} | ${budgetOf(v)} | ${statusOf(v)} |`),
  ];
  if (run) lines.push("", `The run: ${run}`);
  return `${lines.join("\n")}\n`;
}

const MARK = /<!-- scale-last-pass: ([0-9a-f]{7,40}) -->/;

/** The commit of the last pass, as an issue's body keeps it, or undefined. */
export function lastPassIn(body) {
  return MARK.exec(body ?? "")?.[1];
}

/**
 * The issue's body: the report, the commits since the last pass (`commits`, oldest first, as
 * GitHub's compare answers them) and the last pass itself, kept for the next run to read.
 */
export function bodyFor({ report, head, lastPass, commits }) {
  const since = lastPass
    ? [
        `### Commits since the last pass (${lastPass.slice(0, 7)}…${head.slice(0, 7)})`,
        "",
        ...(commits.length > 0 ? commits.map((c) => `- ${c.sha.slice(0, 7)} ${c.subject}`) : ["- none"]),
      ]
    : ["### Commits since the last pass", "", "There is no pass recorded yet: no run has met every budget since this issue was opened."];
  return [
    "One issue for the nightly scale run (`.github/workflows/scale.yml`, #5497): opened or updated when a budget of #5486 is missed, closed by the next run that meets them all.",
    "",
    report.trimEnd(),
    "",
    ...since,
    "",
    lastPass ? `<!-- scale-last-pass: ${lastPass} -->` : "",
  ].join("\n");
}

/** `<rows.jsonl>… [--issue] [--only macos|linux]`, in any order. */
export function parseArgv(argv) {
  const files = [];
  let issue = false;
  let only;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--issue") issue = true;
    else if (argv[i] === "--only") {
      only = argv[++i];
      if (!(only in SYSTEMS)) throw new Error(`--only takes ${Object.keys(SYSTEMS).join(" or ")}, not "${only}"`);
    } else files.push(argv[i]);
  }
  return { files, issue, only };
}

/** Every row of a file `tools/scale-run.mjs` wrote. */
export function parseRows(text) {
  return text
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line) => JSON.parse(line));
}

// The I/O: everything above is pure. The calls are a handful a night: the issue looked for, the
// label made once, a compare, the issue written and a comment.

export const ISSUE_TITLE = "Scale run: a budget is missed";
export const ISSUE_LABEL = "scale-regression";

const API = process.env.GITHUB_API_URL ?? "https://api.github.com";

async function call(path, { method = "GET", body, token, missing } = {}) {
  for (let attempt = 1; ; attempt += 1) {
    const response = await fetch(`${API}${path}`, {
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
    if (missing && response.status === 404) return missing;
    const text = await response.text();
    if (attempt < 3 && (response.status >= 500 || response.status === 429)) {
      await new Promise((resolve) => setTimeout(resolve, attempt * 10_000));
      continue;
    }
    throw new Error(`${method} ${path}: ${response.status} ${text.slice(0, 300)}`);
  }
}

async function keepIssue(found, report, { repo, head, token }) {
  const listed = await call(`/repos/${repo}/issues?labels=${ISSUE_LABEL}&state=all&per_page=100`, {
    token,
    missing: [],
  });
  const issue = listed.find((i) => i.title === ISSUE_TITLE && !i.pull_request);
  const decided = decide(found, issue);
  const lastPass = lastPassIn(issue?.body);
  const headline = summary(found).split("\n").find((line) => line.startsWith("**")) ?? "";
  if (decided.action === "none") return decided;
  if (decided.action === "close") {
    const body = bodyFor({ report, head, lastPass: head, commits: [] });
    await call(`/repos/${repo}/issues/${decided.number}/comments`, {
      method: "POST",
      body: { body: `Every budget met at ${head}: closed by the scale run.` },
      token,
    });
    await call(`/repos/${repo}/issues/${decided.number}`, { method: "PATCH", body: { body, state: "closed" }, token });
    return decided;
  }
  const compared = lastPass ? await call(`/repos/${repo}/compare/${lastPass}...${head}`, { token }) : { commits: [] };
  const commits = compared.commits.map((c) => ({ sha: c.sha, subject: c.commit.message.split("\n")[0] }));
  const body = bodyFor({ report, head, lastPass, commits });
  let numbered = decided.number;
  if (numbered === undefined) {
    // The label first: an issue created with a label that does not exist is refused.
    const label = await call(`/repos/${repo}/labels/${ISSUE_LABEL}`, { token, missing: null });
    if (label === null) {
      await call(`/repos/${repo}/labels`, {
        method: "POST",
        body: { name: ISSUE_LABEL, color: "b60205", description: "The nightly scale run missed a budget (#5497)" },
        token,
      });
    }
    numbered = (await call(`/repos/${repo}/issues`, { method: "POST", body: { title: ISSUE_TITLE, body, labels: [ISSUE_LABEL] }, token })).number;
  } else {
    await call(`/repos/${repo}/issues/${numbered}`, { method: "PATCH", body: { body, state: "open" }, token });
  }
  await call(`/repos/${repo}/issues/${numbered}/comments`, { method: "POST", body: { body: `${headline.replaceAll("*", "")} At ${head}.` }, token });
  return { ...decided, number: numbered };
}

async function main(argv) {
  const { files, issue, only } = parseArgv(argv);
  const rows = files.flatMap((file) => parseRows(readFileSync(file, "utf8")));
  const found = verdicts(rows, { only });
  const head = process.env.GITHUB_SHA ?? "";
  const run = process.env.GITHUB_RUN_ID
    ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}`
    : undefined;
  const report = summary(found, { head, run });
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, report);
  else process.stdout.write(report);
  for (const v of found.filter((v) => v.status === "missed")) {
    console.log(`::warning title=Scale budget missed (${SYSTEMS[v.os]})::${v.spec.what}: ${shown(v)} against ${budgetOf(v)}`);
  }
  if (issue) {
    const token = process.env.GITHUB_TOKEN;
    if (!token) throw new Error("--issue needs GITHUB_TOKEN");
    const kept = await keepIssue(found, report, { repo: process.env.GITHUB_REPOSITORY, head, token });
    console.log(`scale-report: the ${ISSUE_LABEL} issue: ${kept.action}${kept.number ? ` #${kept.number}` : ""}`);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main(process.argv.slice(2)).catch((err) => {
    console.error(`scale-report: ${err.message}`);
    process.exit(1);
  });
}
