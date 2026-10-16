// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Each scenario test that failed, as a GitHub annotation on its spec's line.
//
// Run by the `scenario` job when it fails: `node ../tools/e2e-annotations.mjs logs/failures.jsonl`
// from `app/`. The scenario configs' `afterTest` and `afterHook` write one line of JSON per
// failure into `app/logs/failures.jsonl` (`app/e2e/failures.ts`); this turns them into
// `::error file=…,line=…,title=…::message` lines. An annotation is read through the checks API,
// which is the one place a failure is visible without the job's log or its artifact.
//
// A mass failure stays readable: at most LIMIT failures, then a notice that counts the rest.
// GitHub keeps only ten error and ten warning annotations per step, so the first ten failures
// are errors and the next ten warnings (titled "also failed"); past that they would be dropped
// silently, and the count is the one line that still says how many there were. A job
// that failed with no test recorded (the build, a step that is not WebdriverIO, an app that never
// started a spec) gets one annotation that says so.

import { existsSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, join, relative, sep } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** The repository's root: annotations name files relative to it. */
const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** How many failures are annotated one by one: ten errors, then ten warnings. */
export const LIMIT = 20;

/** GitHub's own limit on annotations of one level in one step. */
const PER_LEVEL = 10;

/** How much of an assertion's message an annotation keeps. */
export const MESSAGE_CHARS = 300;

/**
 * How many of the first failures carry the evidence the job's log and artifact hold (#1949's
 * scenario stall): what the window was still waiting for, and the core's last log lines. The
 * first failure of a mass failure is the one that names its cause.
 */
export const EVIDENCE_FOR = 5;

/** How much evidence one annotation keeps, the newest lines kept when it is cut. */
export const EVIDENCE_CHARS = 3000;

/** Whether `record` carries the evidence block: a test's or a hook's failure does, the left-chats
 *  guard's own record does not. */
export function carriesEvidence(record) {
  return Array.isArray(record.late) || Array.isArray(record.coreLog);
}

/**
 * The evidence `record` carries, as lines of text: the commands the window had not had answered
 * in time (`app/src/answerWatch.ts`) and the core's last log lines, which the app already writes
 * with anything secret-shaped replaced (`purlis_core::applog`). Empty where it carries none.
 */
export function evidenceOf(record) {
  if (!carriesEvidence(record)) return "";
  let late = "";
  const waited = Array.isArray(record.late) ? record.late : [];
  if (waited.length > 0) {
    const said = waited.map((one) =>
      typeof one.ms === "number"
        ? `${one.command} (answered after ${Math.round(one.ms / 1000)} s)`
        : `${one.command} (never answered)`,
    );
    late = `the window waited for: ${said.join(", ")}`;
  }
  // What lay over the element a test could not press or see (`app/e2e/covers.ts`). They are read
  // from the page, which the core's redaction never saw, so they go through the same check here.
  const over = Array.isArray(record.covers) ? record.covers : [];
  const covers =
    over.length > 0
      ? `what covered it:\n${over.map((line) => redacted(String(line))).join("\n")}`
      : "";
  const log = Array.isArray(record.coreLog) ? record.coreLog : [];
  let logPart = "";
  if (log.length > 0) {
    const header = "the core's last log lines:\n";
    // The late commands and the covers are kept whole; the log has the room they leave, its
    // newest lines kept, since they say what happened last.
    const room =
      EVIDENCE_CHARS -
      [late, covers]
        .filter(Boolean)
        .reduce((n, part) => n + part.length + 1, 0) -
      header.length;
    let lines = log.map((line) => String(line).replace(ANSI, ""));
    while (lines.length > 1 && lines.join("\n").length > room)
      lines = lines.slice(1);
    // One line longer than the room on its own keeps its newest end, cut to the room, so the
    // cut below never reaches the late commands; with no room left, the log goes.
    const kept = lines.join("\n");
    if (room <= 0) logPart = "";
    else
      logPart = `${header}${kept.length > room ? kept.slice(kept.length - room) : kept}`;
  }
  const parts = [late, logPart, covers].filter(Boolean);
  // A block recorded with nothing in it says so: a quiet core reads apart from no block at all.
  if (parts.length === 0)
    parts.push("no command was late; the core logged nothing");
  const text = parts.join("\n");
  // Only late commands and covers past the budget on their own still need cutting: the newest
  // end is kept.
  return text.length > EVIDENCE_CHARS
    ? text.slice(text.length - EVIDENCE_CHARS)
    : text;
}

/**
 * `line`, or in its place a line naming the kind of secret it looks like it holds: the shapes
 * the core's log is checked for (`purlis_core::applog`, `secretshape`), the common ones, for
 * text the core never saw. Like the log, the whole line goes, not only the match.
 */
export function redacted(line) {
  for (const [shape, kind] of SECRET_SHAPES)
    if (shape.test(line))
      return `[redacted: a line that looks like it holds ${kind}]`;
  return line;
}

const SECRET_SHAPES = [
  [
    /\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}|\bgithub_pat_[A-Za-z0-9_]{20,}/,
    "a GitHub token",
  ],
  [/\bglpat-[A-Za-z0-9_-]{20,}/, "a GitLab token"],
  [/\bsk-(?:ant-|proj-)?[A-Za-z0-9_-]{20,}/, "an API key"],
  [/\bxox[abposr]-[A-Za-z0-9-]{10,}/, "a Slack token"],
  [/\b(?:AKIA|ASIA)[A-Z0-9]{16}\b/, "an AWS key"],
  [/-----BEGIN [A-Z ]*PRIVATE KEY-----/, "a private key"],
  [
    /\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/,
    "a JSON web token",
  ],
  [/\bBearer\s+[A-Za-z0-9._~+/=-]{16,}/i, "a bearer token"],
  [
    /[A-Za-z_]*(?:token|secret|password|passwd|api[_-]?key)[A-Za-z_]*["']?\s*[:=]\s*["']?[^\s"']{6,}/i,
    "a field named for a secret",
  ],
  [/\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/, "an email address"],
];

// Colour codes the spec reporter and `expect` put in their messages.
// eslint-disable-next-line no-control-regex
const ANSI = /\u001b\[[0-9;]*m/g;

/** The records in a `failures.jsonl`, skipping any line that is not JSON (a write cut short). */
export function readRecords(text) {
  const records = [];
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    try {
      records.push(JSON.parse(line));
    } catch {
      // A worker killed mid-write leaves half a line; the other records still count.
    }
  }
  return records;
}

/** `file` as GitHub wants it: relative to the repository, with forward slashes. */
function repoPath(file, root) {
  const plain = file.startsWith("file://") ? fileURLToPath(file) : file;
  const rel = isAbsolute(plain) ? relative(root, plain) : plain;
  return rel.split(sep).join("/");
}

/**
 * The line in `file` the failure points at: the first stack frame in that file, or else the
 * line that names the test's title, or else 1.
 */
export function lineOf(record, root, read = readSource) {
  const file = record.file ?? "";
  const base = file.startsWith("file://") ? fileURLToPath(file) : file;
  if (base) {
    for (const frame of String(record.stack ?? "").split("\n")) {
      const at = frame.indexOf(base);
      if (at < 0) continue;
      const match = /^:(\d+)/.exec(frame.slice(at + base.length));
      if (match) return Number(match[1]);
    }
    const source = read(isAbsolute(base) ? base : join(root, base));
    if (source && record.title) {
      const index = source
        .split("\n")
        .findIndex((line) => line.includes(record.title));
      if (index >= 0) return index + 1;
    }
  }
  return 1;
}

function readSource(path) {
  try {
    return existsSync(path) ? readFileSync(path, "utf8") : "";
  } catch {
    return "";
  }
}

/** The message an annotation carries: no colour codes, collapsed to MESSAGE_CHARS. */
export function trimMessage(message) {
  const plain = String(message ?? "")
    .replace(ANSI, "")
    .trim();
  if (plain.length <= MESSAGE_CHARS) return plain;
  return `${plain.slice(0, MESSAGE_CHARS - 1)}…`;
}

// Workflow command escaping: `%`, CR and LF in data; `:` and `,` too in a property.
export const escapeData = (s) =>
  s.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
export const escapeProperty = (s) =>
  escapeData(s).replace(/:/g, "%3A").replace(/,/g, "%2C");

/**
 * The `::error` lines for `records`, at most `limit` of them and a last one counting the rest.
 *
 * @param records what `app/e2e/failures.ts` wrote: `{ file, title, parent, message, stack }`.
 */
export function annotations(
  records,
  { root = ROOT, limit = LIMIT, read = readSource } = {},
) {
  if (records.length === 0) {
    return [
      "::error title=scenario tests failed with no test recorded::" +
        escapeData(
          "No scenario test recorded a failure: a build step, the relaunch test or the app's start failed. The step marked failed in this job says which.",
        ),
    ];
  }
  let evidenced = 0;
  const lines = records.slice(0, limit).map((record, index) => {
    const level = index < PER_LEVEL ? "error" : "warning";
    const props = [];
    if (record.file) {
      props.push(`file=${escapeProperty(repoPath(record.file, root))}`);
      props.push(`line=${lineOf(record, root, read)}`);
    }
    const named =
      [record.parent, record.title].filter(Boolean).join(" › ") ||
      "scenario test";
    const title = level === "error" ? named : `also failed › ${named}`;
    props.push(`title=${escapeProperty(title)}`);
    const trimmed = trimMessage(record.message) || "failed with no message";
    // The first EVIDENCE_FOR failures that carry a block show it; a record with none uses no place.
    const evidence =
      carriesEvidence(record) && evidenced++ < EVIDENCE_FOR
        ? evidenceOf(record)
        : "";
    const message = evidence ? `${trimmed}\n\n${evidence}` : trimmed;
    return `::${level} ${props.join(",")}::${escapeData(message)}`;
  });
  const rest = records.length - lines.length;
  if (rest > 0) {
    lines.push(
      `::notice title=+${rest} more failed scenario tests::${escapeData(
        `${rest} more failed; ${records.length} in all. The first ${lines.length} are annotated.`,
      )}`,
    );
  }
  return lines;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const file = process.argv[2] ?? "logs/failures.jsonl";
  const records = existsSync(file)
    ? readRecords(readFileSync(file, "utf8"))
    : [];
  for (const line of annotations(records)) console.log(line);
}
