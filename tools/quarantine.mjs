// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The quarantine (spec #7324, ADR 0127): `ci/quarantine.toml` lists the tests that flaked three
// times in seven days, and every runner skips them except in the nightly run.
//
//   node tools/quarantine.mjs nextest   the filterset the `rust` job's nextest run adds
//   node tools/quarantine.mjs check     refuses an entry without a ticket or a date
//
// The id of an entry is the id the flake was annotated with (`tools/flake-annotation.mjs`), so
// ci-health can copy a flake into the list: nextest's is `<binary id> <test name>`, vitest's is
// `<file> > <suite> > <test>`, wdio's is the spec file. The file is read here by a reader of the
// one shape it has (`[[test]]` tables of `id`, `ticket` and `added`), which refuses anything else.

import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** The checked-in list, from the repository's root. */
export const QUARANTINE_FILE = join(dirname(fileURLToPath(import.meta.url)), "..", "ci", "quarantine.toml");

/** The most tests quarantined at once: past it ci-health goes red (#7333). */
export const CAP = 10;

/**
 * The entries of a quarantine file, `{ id, ticket, added }` each (a key missing is `undefined`, for
 * `problemsWith` to name). Throws on a line that is none of a comment, a blank, `[[test]]` or a
 * `key = value` of those three keys: the reader knows no other TOML.
 */
export function parseQuarantine(text) {
  const entries = [];
  for (const [i, raw] of text.split("\n").entries()) {
    const line = raw.replace(/^\s*#.*$/, "").trim();
    if (line === "") continue;
    if (line === "[[test]]") {
      entries.push({});
      continue;
    }
    const pair = /^(id|ticket|added)\s*=\s*(?:"((?:[^"\\]|\\.)*)"|(\d+))\s*(?:#.*)?$/.exec(line);
    if (!pair || entries.length === 0) throw new Error(`ci/quarantine.toml line ${i + 1}: cannot read "${raw.trim()}"`);
    const [, key, quoted, number] = pair;
    const value = quoted !== undefined ? quoted.replace(/\\(["\\])/g, "$1") : Number(number);
    const entry = entries.at(-1);
    if (key in entry) throw new Error(`ci/quarantine.toml line ${i + 1}: ${key} twice in one entry`);
    entry[key] = value;
  }
  return entries;
}

/** What is wrong with `entries`, one sentence each: a test without its ticket or its date, or twice. */
export function problemsWith(entries) {
  const problems = [];
  const seen = new Set();
  for (const [n, entry] of entries.entries()) {
    const name = typeof entry.id === "string" && entry.id !== "" ? `"${entry.id}"` : `entry ${n + 1}`;
    if (typeof entry.id !== "string" || entry.id === "") problems.push(`${name} has no id`);
    else if (seen.has(entry.id)) problems.push(`${name} is listed twice`);
    else seen.add(entry.id);
    if (!Number.isInteger(entry.ticket) || entry.ticket <= 0) problems.push(`${name} has no ticket: name the issue that fixes it`);
    if (typeof entry.added !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(entry.added) || Number.isNaN(Date.parse(entry.added))) {
      problems.push(`${name} has no date: write the day it was added as yyyy-mm-dd`);
    }
  }
  return problems;
}

/** Which runner an id is for, by its shape: wdio's is a spec file, vitest's holds ` > `. */
export function runnerOf(id) {
  if (/^\S+\.e2e\.ts$/.test(id)) return "wdio";
  if (id.includes(" > ")) return "vitest";
  return "nextest";
}

/** The file read: `PURLIS_QUARANTINE_FILE` when set (the tests', a fixture's), else the checked-in one. */
export function quarantineFile(env = process.env) {
  return env.PURLIS_QUARANTINE_FILE || QUARANTINE_FILE;
}

/** The entries of the file `env` names, none when it is missing. */
export function readQuarantine(env = process.env) {
  const file = quarantineFile(env);
  return existsSync(file) ? parseQuarantine(readFileSync(file, "utf8")) : [];
}

/**
 * Whether this run skips the quarantine: every run of CI but the nightly's (a `schedule` event).
 * The pull request run, the queue run and the main run skip it; a person's machine, where `CI` is
 * not set, runs every test, because a red there is the answer they are waiting for.
 */
export function skipsQuarantine(env = process.env) {
  return Boolean(env.CI) && env.GITHUB_EVENT_NAME !== "schedule";
}

/** The ids of `runner` this run skips: none in the nightly run, or off CI. */
export function skippedIds(runner, env = process.env) {
  if (!skipsQuarantine(env)) return [];
  return readQuarantine(env)
    .map((entry) => entry.id)
    .filter((id) => runnerOf(id) === runner);
}

/**
 * The nextest filterset that leaves the skipped tests out, to `&` onto a run's own: `all()` when
 * nothing is skipped. A test is named by its binary id and its name, exactly.
 */
export function nextestFilter(env = process.env) {
  const named = skippedIds("nextest", env).map((id) => {
    const at = id.indexOf(" ");
    if (at < 0) throw new Error(`the nextest id "${id}" is a binary id, a space and a test's name`);
    return `(binary_id(=${id.slice(0, at)}) & test(=${id.slice(at + 1)}))`;
  });
  return named.length === 0 ? "all()" : `not (${named.join(" | ")})`;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const [command] = process.argv.slice(2);
  try {
    if (command === "nextest") {
      console.log(nextestFilter());
    } else if (command === "check") {
      const problems = problemsWith(readQuarantine());
      for (const problem of problems) console.error(`::error file=ci/quarantine.toml::${problem}`);
      if (problems.length > 0) process.exitCode = 1;
    } else {
      throw new Error("usage: node tools/quarantine.mjs nextest | check");
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
