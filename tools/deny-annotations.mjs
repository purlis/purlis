// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Each cargo-deny finding, as a GitHub annotation on `Cargo.lock` (qw390).
//
// Run by the `deny` job after `cargo deny --format json check`, on what cargo-deny wrote to
// stderr: `node tools/deny-annotations.mjs deny.jsonl`. cargo-deny writes one JSON object a
// line, a `diagnostic` for each finding (an advisory, a licence, a ban, a source) and a `log`
// for what went wrong around them (an advisory database it could not fetch or read). This turns
// each error into an `::error file=Cargo.lock,title=…::message` line, which the checks API
// returns: the one place a failure is readable without the job's log.
//
// At most ten errors are annotated (GitHub keeps ten error annotations a step, so a higher cap
// would only lose them silently), then a notice counts the rest and points at the job's log,
// where the step prints all of cargo-deny's output. A job that failed with nothing to annotate
// says so in one annotation.

import { existsSync, readFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** How many findings are annotated one by one. */
export const LIMIT = 10;

/** How much of a message an annotation keeps. */
export const MESSAGE_CHARS = 300;

/** The objects cargo-deny wrote, one a line, skipping any line that is not JSON. */
export function readRecords(text) {
  const records = [];
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    try {
      records.push(JSON.parse(line));
    } catch {
      // A plain line (a progress message, a panic): the JSON ones still count.
    }
  }
  return records;
}

/** The crate a diagnostic is about: from its advisory, or the first crate of its graph. */
function crateOf(fields) {
  const advisory = fields.advisory ?? {};
  if (advisory.package) return String(advisory.package);
  const krate = fields.graphs?.[0]?.Krate;
  if (krate?.name) return krate.version ? `${krate.name} ${krate.version}` : String(krate.name);
  return "";
}

/** Each finding that fails the check: `{ title, message }`. */
export function findings(records) {
  const out = [];
  for (const record of records) {
    const fields = record?.fields ?? {};
    if (record?.type === "diagnostic" && String(fields.severity).toLowerCase() === "error") {
      const advisory = fields.advisory ?? {};
      const id = advisory.id ?? fields.code ?? "cargo-deny";
      const crate = crateOf(fields);
      const what = advisory.title ?? fields.message ?? "";
      out.push({
        title: `cargo-deny ${fields.code ?? "finding"}`,
        message: [id, crate, what].filter(Boolean).join(" "),
      });
    } else if (record?.type === "log" && String(fields.level).toUpperCase() === "ERROR") {
      out.push({ title: "cargo-deny error", message: String(fields.message ?? "") });
    }
  }
  return out;
}

/** The message an annotation carries, on one line, at most MESSAGE_CHARS. */
export function trimMessage(message) {
  const plain = String(message ?? "")
    .replace(/\s+/g, " ")
    .trim();
  if (plain.length <= MESSAGE_CHARS) return plain;
  return `${plain.slice(0, MESSAGE_CHARS - 1)}…`;
}

// Workflow command escaping: `%`, CR and LF in data; `:` and `,` too in a property.
const escapeData = (s) => s.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
const escapeProperty = (s) => escapeData(s).replace(/:/g, "%3A").replace(/,/g, "%2C");

/** The `::error` lines for `records`, at most `limit`, then a notice counting the rest. */
export function annotations(records, { limit = LIMIT } = {}) {
  const found = findings(records);
  if (found.length === 0) {
    return [
      "::error file=Cargo.lock,title=cargo-deny failed with no finding recorded::" +
        escapeData(
          "cargo-deny reported no error finding: it did not start, or failed before it checked. The step marked failed in this job says which.",
        ),
    ];
  }
  const lines = found
    .slice(0, limit)
    .map(
      ({ title, message }) =>
        `::error file=Cargo.lock,title=${escapeProperty(title)}::${escapeData(trimMessage(message) || "no message")}`,
    );
  const rest = found.length - lines.length;
  if (rest > 0) {
    lines.push(
      `::notice title=+${rest} more cargo-deny findings::${escapeData(
        `${rest} more; ${found.length} in all. The first ${lines.length} are annotated; the job's log has every one.`,
      )}`,
    );
  }
  return lines;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const file = process.argv[2] ?? "deny.jsonl";
  const records = existsSync(file) ? readRecords(readFileSync(file, "utf8")) : [];
  for (const line of annotations(records)) console.log(line);
}
