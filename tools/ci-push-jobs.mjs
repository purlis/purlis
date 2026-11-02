#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **Every job that runs on a push to main has chosen what its red means (#4471).** `ci` on main
// ending `failure` does two things: `release.yml` publishes no dev build, and `tools/land.mjs`
// refuses to land. Both read the run's conclusion, and any job can turn that red, required or
// not: `outcome bars`, which no check required, stopped both on 2026-11-01. So each job of
// `ci.yml` that can run on a push is one of:
//
// - **required**: a required check names it, or a job that is reached reads its result. A job a
//   required job only `needs` is not reached: with `!cancelled()` that job runs and passes
//   whatever the need did, and with no `if:` it is skipped, and a skipped check counts as passing.
//   Only `needs.<job>.result` (or `needs.*.result`) reads the answer.
// - **evidence**: `continue-on-error` is true on a push, so its red reaches neither the queue nor
//   the run (V70, ADR 0086 as amended 2026-10-04).
// - **listed**: in `LISTED`, with the reason it may stop landing while no check requires it.
//
// `ci-push-jobs.test.mjs` runs this on `ci.yml`, in the tools' tests of `web`'s part 2, which
// `web (types, lint, format, test)` reads, so a job that has not chosen fails a pull request.
// `node tools/ci-push-jobs.mjs [workflow]` prints every job's standing.
import { readFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

/**
 * The checks main requires that `ci.yml` posts, as branch protection names them. The list is held
 * here because a pull request's run cannot be made to depend on a read of protection: `ci-health`
 * compares it with main's rules each week and says when they part.
 */
export const REQUIRED_CHECKS = [
  "app builds (macos-latest)",
  "app builds (ubuntu-24.04)",
  "recorded behaviour",
  "rust (fmt, clippy, test)",
  "rust (licences, advisories)",
  "scenario tests (macos-latest)",
  "scenario tests (ubuntu-24.04)",
  "web (types, lint, format, test)",
];

/** The checks main requires that another workflow posts: they say nothing of `ci.yml`. */
export const REQUIRED_ELSEWHERE = ["docs (build, links, reachable)"];

/** Jobs that may stop landing and releases with no required check behind them, and why. */
export const LISTED = {
  changes:
    "A fault here is a fault in the scope tool: every scope then runs, so nothing goes unchecked, and on a push every scope runs anyway. It stays red on main for a person to fix the tool.",
  parity:
    "A real gate on the host, protocol, sandbox and browsers. The operator ruled that it be required, and branch protection has yet to name it (#4335). Its red on main is a regression a person should act on, so it stops landing until then; add it to REQUIRED_CHECKS and take this line off with that.",
};

const WORDS = /'(?:[^']|'')*'|&&|\|\||==|!=|[!(),]|[\w.*\-[\]]+/g;
const UNKNOWN = undefined;

/** What a push to main knows. Anything else (`needs.*`, `matrix.*`) depends on the run. */
const PUSH = { "github.event_name": "push", "github.ref": "refs/heads/main" };
const CALLS = { cancelled: false, always: true };

const truth = (value) => (value === UNKNOWN ? UNKNOWN : Boolean(value));

/**
 * A GitHub expression read as a push to main reads it: true, false, or `undefined` for what only
 * the run can know. An unknown stays unknown through `!`, and through `&&` and `||` unless the
 * other side settles it, so only a known false can take a job off a push. What this cannot parse
 * is unknown too.
 */
export function evaluate(source) {
  const inner = /^\s*\$\{\{(.*)\}\}\s*$/s.exec(source);
  const words = (inner ? inner[1] : source).match(WORDS) ?? [];
  let at = 0;
  const atom = () => {
    const word = words[at++];
    if (word === undefined) throw new Error("the expression ended early");
    if (word === "(") {
      const value = or();
      if (words[at++] !== ")") throw new Error("a bracket did not close");
      return value;
    }
    if (word === "!") {
      const value = truth(atom());
      return value === UNKNOWN ? UNKNOWN : !value;
    }
    if (word.startsWith("'")) return word.slice(1, -1).replaceAll("''", "'");
    if (word === "true") return true;
    if (word === "false") return false;
    if (words[at] === "(") {
      at += 1;
      while (words[at] !== ")") {
        if (words[at] === ",") at += 1;
        else or();
      }
      at += 1;
      return word in CALLS ? CALLS[word] : UNKNOWN;
    }
    return PUSH[word];
  };
  const compare = () => {
    const left = atom();
    if (words[at] !== "==" && words[at] !== "!=") return left;
    const equal = words[at++] === "==";
    const right = atom();
    if (left === UNKNOWN || right === UNKNOWN) return UNKNOWN;
    return (left === right) === equal;
  };
  const and = () => {
    let value = compare();
    while (words[at] === "&&") {
      at += 1;
      const right = compare();
      if (truth(value) === false) continue;
      value = truth(right) === false ? false : value === UNKNOWN ? UNKNOWN : right;
    }
    return value;
  };
  const or = () => {
    let value = and();
    while (words[at] === "||") {
      at += 1;
      const right = and();
      if (truth(value) === true) continue;
      value = truth(right) === true ? right : value === UNKNOWN ? UNKNOWN : right;
    }
    return value;
  };
  try {
    const value = or();
    return at === words.length ? truth(value) : UNKNOWN;
  } catch {
    return UNKNOWN;
  }
}

/** Whether a job can run on a push: only an `if:` known to be false says it cannot. */
export const onPush = (job) => job.if === undefined || evaluate(job.if) !== false;

const scalar = (value) => value.trim().replace(/^(["'])(.*)\1$/, "$2");

/** A key's value on its own line, or the block scalar or list under it at `lines[at + 1]`. */
function valueAt(lines, at, value) {
  const rest = value.trim();
  if (rest !== "" && !/^[|>][-+]?$/.test(rest)) return rest;
  const under = [];
  for (let i = at + 1; i < lines.length && /^ {6}/.test(lines[i]); i += 1) under.push(lines[i].trim());
  return rest === "" ? under : under.join(" ");
}

function needsOf(value) {
  if (value === undefined) return [];
  if (Array.isArray(value)) return value.map((item) => item.replace(/^- /, "").trim());
  return value.replace(/^\[|\]$/g, "").split(",").map((item) => item.trim()).filter(Boolean);
}

/**
 * The jobs of a workflow's text: `{ id, name, needs, if, continueOnError, reads, text }`, where
 * `reads` is the jobs whose result it reads. Line by line, as the other tools read workflows: the
 * tools' tests have no packages to load. A workflow written so that no job can be read is an
 * error rather than a pass.
 */
export function jobsOf(text) {
  const lines = text.split("\n");
  const start = lines.indexOf("jobs:");
  if (start < 0) throw new Error("no jobs: this workflow has no `jobs:` to read");
  const jobs = [];
  let job;
  const body = [];
  const close = () => {
    if (!job) return;
    const code = body.filter((line) => !/^\s*#/.test(line));
    const key = (name) => {
      const at = code.findIndex((line) => line.startsWith(`    ${name}:`));
      return at < 0 ? undefined : valueAt(code, at, code[at].slice(`    ${name}:`.length));
    };
    const source = code.join("\n");
    jobs.push({
      ...job,
      name: scalar(key("name") ?? job.id),
      needs: needsOf(key("needs")),
      if: key("if"),
      continueOnError: key("continue-on-error"),
      reads: needsOf(key("needs")).filter(
        (need) => source.includes(`needs.${need}.result`) || source.includes("needs.*.result"),
      ),
      text: source,
    });
    body.length = 0;
  };
  for (const line of lines.slice(start + 1)) {
    if (/^\S/.test(line)) break;
    const id = /^ {2}([\w-]+):\s*$/.exec(line);
    if (id) {
      close();
      job = { id: id[1] };
    } else if (job) body.push(line);
  }
  close();
  if (jobs.length === 0) throw new Error("no jobs: nothing under `jobs:` could be read");
  return jobs;
}

/** A job's name as a pattern: `${{ }}` in it stands for whatever the matrix writes. */
const nameMatches = (name, check) =>
  new RegExp(
    `^${name
      .split(/\$\{\{.*?\}\}/)
      .map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
      .join(".*")}$`,
  ).test(check);

const REFUSED = (job) =>
  `Job "${job.id}" runs on a push to main and nothing requires it. When it fails, ci on main ` +
  "ends in failure: tools/land.mjs stops all landing and release.yml publishes no dev build. " +
  "Choose one: make it required (a job a required check reaches reads needs." +
  `${job.id}.result; only needing it is not enough), make it evidence (continue-on-error: true), ` +
  "or add it to LISTED in tools/ci-push-jobs.mjs with the reason it may stop landing (#4471).";

/**
 * Every job of `text` by its standing on a push: `required`, `reached`, `evidence`, `listed`,
 * `absent` (it never runs on one), and `refused`, each refused one with its `why`. `listed` is
 * `{ job id: reason }`, and a listing that has no reason or does nothing is refused too.
 */
export function sortJobs(text, { required, listed }) {
  const jobs = jobsOf(text);
  const byId = new Map(jobs.map((job) => [job.id, job]));
  const named = jobs.filter((job) => required.some((check) => nameMatches(job.name, check)));
  const seen = new Set(named.map((job) => job.id));
  for (const id of seen) for (const need of byId.get(id).reads) seen.add(need);

  const out = { required: [], reached: [], evidence: [], listed: [], absent: [], refused: [] };
  for (const job of jobs) {
    const reason = listed[job.id];
    const isListed = reason !== undefined;
    const evidence = job.continueOnError !== undefined && evaluate(job.continueOnError) === true;
    const fine = named.includes(job) || seen.has(job.id) || evidence;
    if (!onPush(job)) {
      if (isListed) out.refused.push({ ...job, why: `Job "${job.id}" is in LISTED but never runs on a push: take it off.` });
      else out.absent.push(job);
    } else if (fine && isListed) {
      out.refused.push({ ...job, why: `Job "${job.id}" is in LISTED but is required or evidence now: take it off.` });
    } else if (named.includes(job)) out.required.push(job);
    else if (seen.has(job.id)) out.reached.push(job);
    else if (evidence) out.evidence.push(job);
    else if (isListed && reason.trim() !== "") out.listed.push({ ...job, reason });
    else out.refused.push({ ...job, why: REFUSED(job) });
  }
  for (const id of Object.keys(listed)) {
    if (!byId.has(id)) out.refused.push({ id, why: `"${id}" is in LISTED but ci.yml has no such job: take it off.` });
  }
  for (const check of required) {
    if (!jobs.some((job) => nameMatches(job.name, check))) {
      out.refused.push({ id: check, why: `The required check "${check}" has no job to post it, so nothing gates it: name a job for it, or take it off REQUIRED_CHECKS and branch protection.` });
    }
  }
  return out;
}

/**
 * What `live` (the checks main requires now, from `requiredNames` in `land.mjs`) and this file's
 * lists disagree on: `{ missing, extra }`. `missing` is required on main and unknown here.
 */
export function requiredDrift(live) {
  const known = [...REQUIRED_CHECKS, ...REQUIRED_ELSEWHERE];
  return {
    missing: live.filter((check) => !known.includes(check)),
    extra: known.filter((check) => !live.includes(check)),
  };
}

function main(argv) {
  const path = argv[0] ?? fileURLToPath(new URL("../.github/workflows/ci.yml", import.meta.url));
  const sorted = sortJobs(readFileSync(path, "utf8"), { required: REQUIRED_CHECKS, listed: LISTED });
  for (const standing of ["required", "reached", "evidence", "listed", "absent"]) {
    for (const job of sorted[standing]) console.log(`${standing}  ${job.id}${job.reason ? `: ${job.reason}` : ""}`);
  }
  for (const job of sorted.refused) console.error(job.why);
  process.exitCode = sorted.refused.length > 0 ? 1 : 0;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main(process.argv.slice(2));
