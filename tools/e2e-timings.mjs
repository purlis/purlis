// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The slowest scenario spec files and tests of a CI run, as two GitHub notices.
//
// Run by the `scenario` jobs after the specs: `node ../tools/e2e-timings.mjs logs/timings.jsonl
// "<label>"` from `app/`. The scenario config's `afterTest` and `afterSession` write one line of
// JSON per test and per spec file's session into `app/logs/timings.jsonl` (`app/e2e/timings.ts`);
// this ranks them. A notice is read through the checks API, the one place a run's numbers are
// visible without its log: the step timings say the specs take most of the job, these say which.
//
// The specs run one at a time in one app, so a spec file's time is the wait itself. Its session
// time includes its hooks, which a test's own time does not.

import { existsSync, readFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** How many spec files, and how many tests, each notice lists. */
export const LIMIT = 30;

/** `file` from its `e2e/` on, as a spec is named in the repository's `app/`. */
function specPath(file) {
  const plain = String(file ?? "").replace(/^file:\/\//, "");
  const at = plain.lastIndexOf("/e2e/");
  return at >= 0 ? plain.slice(at + 1) : plain;
}

/** The records in a `timings.jsonl`, skipping any line that is not JSON (a write cut short). */
export function readTimings(text) {
  const tests = [];
  const specs = [];
  const bars = [];
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    let record;
    try {
      record = JSON.parse(line);
    } catch {
      continue;
    }
    const seconds = (Number(record.ms) || 0) / 1000;
    if (record.kind === "test" || record.kind === "hook") {
      // A hook's title is WebdriverIO's own: `"before all" hook for …`.
      const name = [record.parent, record.title].filter(Boolean).join(" › ");
      tests.push({ file: specPath(record.file), name, seconds });
    } else if (record.kind === "spec") {
      specs.push({ file: specPath(record.file), seconds });
    } else if (record.kind === "bar") {
      // A speed bar measured, not asserted (#4419): `ms` is null when the app refused past it.
      const ms = typeof record.ms === "number" ? record.ms : null;
      const bar = Number(record.bar) || 0;
      bars.push({
        name: String(record.name ?? ""),
        ms,
        bar,
        refused: record.refused,
        met: ms !== null && ms <= bar,
      });
    }
  }
  return { tests, specs, bars };
}

// Workflow command escaping: `%`, CR and LF in data; `:` and `,` too in a property.
const escapeData = (s) =>
  s.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
const escapeProperty = (s) =>
  escapeData(s).replace(/:/g, "%3A").replace(/,/g, "%2C");

const command = (level, title, lines) =>
  `::${level} title=${escapeProperty(title)}::${escapeData(lines.join("\n"))}`;
const notice = (title, lines) => command("notice", title, lines);
const time = (seconds) => `${seconds.toFixed(1)} s`.padStart(8);
const slowestFirst = (list, limit) =>
  [...list].sort((a, b) => b.seconds - a.seconds).slice(0, limit);

/**
 * **The speed bars a run measured** (#4419): a bar a scenario holds the app to on a person's
 * machine, which a loaded CI runner misses for reasons of its own. The spec waits on the state it
 * needs and records the time here; a miss is a `::warning`, which is read through the checks API
 * like the notices and never fails the job. Nothing when no bar was measured.
 */
function barLines(bars, label) {
  if (bars.length === 0) return [];
  const seconds = (ms) => `${(ms / 1000).toFixed(1)} s`;
  const lines = bars.map(
    (one) =>
      `${one.met ? "met    " : "missed "} ${one.ms === null ? "refused" : seconds(one.ms)} against ` +
      `${seconds(one.bar)}  ${one.name}${one.refused ? `: ${one.refused}` : ""}`,
  );
  return bars.every((one) => one.met)
    ? [notice(`Speed bars (${label})`, lines)]
    : [command("warning", `Speed bars missed (${label})`, lines)];
}

/**
 * The two `::notice` lines, titled with `label` (the job's part), or one that says there were
 * none; then the speed bars, if any were measured.
 */
export function annotations(
  { tests, specs, bars = [] },
  { label, limit = LIMIT },
) {
  const files = `Slowest scenario spec files (${label})`;
  if (tests.length === 0 && specs.length === 0)
    return [
      notice(files, ["No scenario timings were recorded."]),
      ...barLines(bars, label),
    ];
  return [
    notice(
      files,
      slowestFirst(specs, limit).map(
        (one) => `${time(one.seconds)}  ${one.file}`,
      ),
    ),
    notice(
      `Slowest scenario tests (${label})`,
      slowestFirst(tests, limit).map(
        (one) => `${time(one.seconds)}  ${one.file}  ${one.name}`,
      ),
    ),
    ...barLines(bars, label),
  ];
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const file = process.argv[2] ?? "logs/timings.jsonl";
  const label = process.argv[3] ?? "scenario";
  const timings = readTimings(
    existsSync(file) ? readFileSync(file, "utf8") : "",
  );
  for (const line of annotations(timings, { label })) console.log(line);
}
