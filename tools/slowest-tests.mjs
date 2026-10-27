// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The slowest Rust tests and test binaries of a CI run, as two GitHub notices.
//
// Run by the `rust` job after its tests: `node tools/slowest-tests.mjs target/nextest/ci/junit.xml`.
// nextest writes that JUnit report (`.config/nextest.toml`, profile `ci`); this reads each
// testcase's binary, name and seconds. A notice is read through the checks API, which is the one
// place a run's numbers are visible without its log: the step timings say the tests take most of
// the job, and these say which tests.
//
// Two notices, each one line per entry: the LIMIT slowest tests, and the LIMIT binaries whose
// tests took longest in all. Tests run at once, so a binary's sum is machine time, not the wait.
//
// And each failed test as an error, with where it panicked and what it said: a red `rust` job is
// read through the checks API too, never through its log.
//
// And each flaky test as a warning: the `ci` profile retries a failed test once (`.config/
// nextest.toml`), and one that passed on its retry is green in the job but named here, with what
// its failed attempt said, so the retry saves a rerun of the whole run and never hides the flake.
//
// `node tools/slowest-tests.mjs --cargo-test <log>` writes the same errors from `cargo test`'s own
// output, kept in `<log>`: the `mutants` workflow's baseline runs `cargo test`, as cargo-mutants
// does for each mutant, and nextest's report is not there to read (#1892). No notices then:
// `cargo test` times no test.

import { existsSync, readFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** How many tests, and how many binaries, each notice lists. */
export const LIMIT = 30;

const ENTITIES = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'" };
const unescape = (s) => s.replace(/&(amp|lt|gt|quot|apos);/g, (_, name) => ENTITIES[name]);

/** Every `<testcase>` in a JUnit report: `{ binary, name, seconds }`, in the report's order. */
export function readCases(xml) {
  const cases = [];
  for (const [, attrs] of xml.matchAll(/<testcase\b([^>]*)>/g)) {
    const attr = (key) => {
      const match = new RegExp(`\\b${key}="([^"]*)"`).exec(attrs);
      return match ? unescape(match[1]) : "";
    };
    cases.push({ binary: attr("classname"), name: attr("name"), seconds: Number(attr("time")) || 0 });
  }
  return cases;
}

/** How many lines of a failed test's output its error keeps, from where it panicked. */
export const FAILURE_LINES = 12;

/**
 * Every failed `<testcase>` in a JUnit report: `{ binary, name, said }`, `said` being the lines
 * from its first `panicked at` on (or its failure's message, where it never panicked).
 */
export function readFailures(xml) {
  const failed = [];
  for (const [, attrs, body = ""] of xml.matchAll(
    /<testcase\b([^>]*?)(?:\/>|>([\s\S]*?)<\/testcase>)/g,
  )) {
    if (!/<(failure|error)\b/.test(body)) continue;
    const attr = (key) => {
      const match = new RegExp(`\\b${key}="([^"]*)"`).exec(attrs);
      return match ? unescape(match[1]) : "";
    };
    const text = unescape(body.replace(/<[^>]*>/g, "\n"));
    const lines = text.split("\n").map((line) => line.trimEnd());
    const at = lines.findIndex((line) => line.includes("panicked at"));
    const message = /<(?:failure|error)\b[^>]*\bmessage="([^"]*)"/.exec(body);
    const said =
      at >= 0
        ? lines.slice(at, at + FAILURE_LINES).filter((line) => line.trim() !== "")
        : [message ? unescape(message[1]) : "failed"];
    failed.push({ binary: attr("classname"), name: attr("name"), said });
  }
  return failed;
}

/**
 * Every test that failed and then passed on a retry: `{ binary, name, retries, said }`, `retries`
 * being how many attempts failed and `said` the first failed attempt's lines from `panicked at` on.
 * nextest writes each failed attempt of such a test as a `<flakyFailure>` (or `<flakyError>`).
 */
export function readFlaky(xml) {
  const flaky = [];
  for (const [, attrs, body = ""] of xml.matchAll(
    /<testcase\b([^>]*?)(?:\/>|>([\s\S]*?)<\/testcase>)/g,
  )) {
    const attempts = [...body.matchAll(/<flaky(?:Failure|Error)\b[^>]*>([\s\S]*?)<\/flaky(?:Failure|Error)>/g)];
    if (attempts.length === 0 || /<(failure|error)\b/.test(body)) continue;
    const attr = (key) => {
      const match = new RegExp(`\\b${key}="([^"]*)"`).exec(attrs);
      return match ? unescape(match[1]) : "";
    };
    const lines = unescape(attempts[0][1]).split("\n").map((line) => line.trimEnd());
    const at = lines.findIndex((line) => line.includes("panicked at"));
    const said = lines.slice(Math.max(at, 0), Math.max(at, 0) + FAILURE_LINES).filter((line) => line.trim() !== "");
    flaky.push({ binary: attr("classname"), name: attr("name"), retries: attempts.length, said: said.length > 0 ? said : ["failed"] });
  }
  return flaky;
}

/** One `::warning` line per flaky test. */
export function flakyAnnotations(flaky) {
  return flaky.map(
    (one) =>
      `::warning title=${escapeProperty(`Flaky (passed on retry ${one.retries}): ${one.binary} ${one.name}`)}::${escapeData(one.said.join("\n"))}`,
  );
}

/** One `::error` line per failed test. */
export function failureAnnotations(failed) {
  return failed.map(
    (one) =>
      `::error title=${escapeProperty(`Failed: ${one.binary} ${one.name}`)}::${escapeData(one.said.join("\n"))}`,
  );
}

/** What a test binary that ended with no result is filed under, in place of a test's name. */
export const ENDED_EARLY = "(the binary ended before its tests' result)";

/**
 * Every failed test in `cargo test`'s own output: `{ binary, name, said }`, as
 * {@link readFailures} answers for a JUnit report.
 *
 * `binary` is what cargo's `Running` line names (`src/lib.rs`, `tests/<name>.rs`), or
 * `doc-tests <crate>`. `said` is the test's captured output from its first `panicked at` on, or
 * from its start where it never panicked (a doctest that does not compile, a test that answered
 * `Err`). A binary that began its tests and wrote no `test result:` line (an abort, a stack
 * overflow, a signal) is one entry under {@link ENDED_EARLY}, with the last lines it wrote.
 */
export function readCargoFailures(text) {
  // Cargo colours its own lines when `CARGO_TERM_COLOR=always`, into a pipe too.
  const lines = text.replace(/\u001b\[[0-9;]*m/g, "").split("\n").map((line) => line.trimEnd());
  const failed = [];
  /** The binary being read: its name, its lines, the tests it called FAILED, each one's output. */
  let now = null;
  const close = () => {
    if (now === null) return;
    const kept = (from) => from.filter((line) => line.trim() !== "").slice(0, FAILURE_LINES);
    for (const name of now.failed) {
      const wrote = now.wrote.get(name) ?? [];
      const at = wrote.findIndex((line) => line.includes("panicked at"));
      const said = kept(at >= 0 ? wrote.slice(at) : wrote);
      failed.push({ binary: now.binary, name, said: said.length > 0 ? said : ["failed"] });
    }
    if (now.began && !now.ended) {
      const last = now.lines.filter((line) => line.trim() !== "").slice(-FAILURE_LINES);
      failed.push({ binary: now.binary, name: ENDED_EARLY, said: last.length > 0 ? last : ["failed"] });
    }
    now = null;
  };
  /** The test whose captured output is being read, between its `----` line and the next. */
  let writing = null;
  for (const line of lines) {
    const running = /^\s*Running (?:unittests )?(\S+) \(/.exec(line);
    const docs = /^\s*Doc-tests (\S+)/.exec(line);
    if (running || docs) {
      close();
      writing = null;
      now = {
        binary: running ? running[1] : `doc-tests ${docs[1]}`,
        lines: [],
        failed: [],
        wrote: new Map(),
        began: false,
        ended: false,
      };
      continue;
    }
    if (now === null) continue;
    // Cargo's summary of the whole run comes after the last binary and is none of its output.
    if (now.ended && /^error: \d+ targets? failed/.test(line)) {
      close();
      continue;
    }
    now.lines.push(line);
    if (/^running \d+ tests?$/.test(line)) now.began = true;
    if (/^test result: /.test(line)) now.ended = true;
    const verdict = /^test (.+) \.\.\. FAILED$/.exec(line);
    if (verdict) now.failed.push(verdict[1]);
    const block = /^---- (.+) stdout ----$/.exec(line);
    if (block) {
      writing = block[1];
      now.wrote.set(writing, []);
    } else if (line === "failures:") {
      writing = null;
    } else if (writing !== null) {
      now.wrote.get(writing).push(line);
    }
  }
  close();
  return failed;
}

/**
 * The `::error` lines for `cargo test`'s own output: one per failed test, as
 * {@link failureAnnotations} writes them. Where the run failed with no test named (it did not
 * build), one error with cargo's own `error` lines. Nothing for a run in which nothing failed.
 */
export function cargoTestAnnotations(text) {
  const failed = readCargoFailures(text);
  if (failed.length > 0) return failureAnnotations(failed);
  const errors = text
    .replace(/\u001b\[[0-9;]*m/g, "")
    .split("\n")
    .map((line) => line.trimEnd())
    .filter((line) => /^error(\[|:)/.test(line))
    .slice(0, FAILURE_LINES);
  if (errors.length === 0) return [];
  return [
    `::error title=${escapeProperty("cargo test failed with no failed test in its output")}::${escapeData(errors.join("\n"))}`,
  ];
}

/** The `limit` slowest tests, and the `limit` binaries whose tests took longest in all. */
export function slowest(cases, { limit = LIMIT } = {}) {
  const tests = [...cases].sort((a, b) => b.seconds - a.seconds).slice(0, limit);
  const sums = new Map();
  for (const { binary, seconds } of cases) {
    const sum = sums.get(binary) ?? { binary, seconds: 0, tests: 0 };
    sum.seconds += seconds;
    sum.tests += 1;
    sums.set(binary, sum);
  }
  const binaries = [...sums.values()]
    .sort((a, b) => b.seconds - a.seconds)
    .slice(0, limit);
  return { tests, binaries };
}

// Workflow command escaping: `%`, CR and LF in data; `:` and `,` too in a property.
const escapeData = (s) => s.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
const escapeProperty = (s) => escapeData(s).replace(/:/g, "%3A").replace(/,/g, "%2C");

const notice = (title, lines) => `::notice title=${escapeProperty(title)}::${escapeData(lines.join("\n"))}`;
const time = (seconds) => `${seconds.toFixed(1)} s`.padStart(8);

/** The two `::notice` lines for `cases`, or one that says there were none. */
export function annotations(cases, { limit = LIMIT } = {}) {
  if (cases.length === 0) return [notice("Slowest Rust tests", ["No JUnit report with tests was found."])];
  const { tests, binaries } = slowest(cases, { limit });
  return [
    notice(
      "Slowest Rust tests",
      tests.map((one) => `${time(one.seconds)}  ${one.binary}  ${one.name}`),
    ),
    notice(
      "Slowest Rust test binaries (their tests' time summed)",
      binaries.map((one) => `${time(one.seconds)}  ${one.binary} (${one.tests} tests)`),
    ),
  ];
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1] && process.argv[2] === "--cargo-test") {
  const file = process.argv[3] ?? "";
  const text = existsSync(file) ? readFileSync(file, "utf8") : "";
  for (const line of cargoTestAnnotations(text)) console.log(line);
} else if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const file = process.argv[2] ?? "target/nextest/ci/junit.xml";
  const xml = existsSync(file) ? readFileSync(file, "utf8") : "";
  const cases = readCases(xml);
  for (const line of [
    ...failureAnnotations(readFailures(xml)),
    ...flakyAnnotations(readFlaky(xml)),
    ...annotations(cases),
  ]) {
    console.log(line);
  }
}
