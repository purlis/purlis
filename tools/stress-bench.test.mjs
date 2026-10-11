// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The `stress` job's relative rows, read from `app/logs/stress.jsonl` for github-action-benchmark
// (#818). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { benchmarks, median, readSamples } from "./stress-bench.mjs";

/** Looks as `stress.e2e.ts` writes them, kilobytes throughout. */
function look(step, fields) {
  return { at: 1, round: 1, step, harnesses: 0, rssKb: 0, ...fields };
}

const run = [
  look("nothing open", { rssKb: 190_000 }),
  look("10 of 50 open", { harnesses: 10, rssKb: 260_000 }),
  look("fifty open", {
    harnesses: 50,
    rssKb: 400_000,
    webContentKb: 300_000,
    webContentFootprintPeakKb: 1_024_000,
    footprintError: null,
  }),
  look("all closed", { rssKb: 330_000 }),
  look("fifty open", {
    harnesses: 50,
    rssKb: 410_000,
    webContentKb: 320_000,
    webContentFootprintPeakKb: 2_048_000,
    footprintError: null,
  }),
  look("all closed", { rssKb: 334_000 }),
  look("fifty open", {
    harnesses: 50,
    rssKb: 420_000,
    webContentKb: 310_000,
    webContentFootprintPeakKb: null,
    footprintError: "footprint: no such process",
  }),
  look("all closed", { rssKb: 336_000 }),
];

const jsonl = [...run.map((r) => JSON.stringify(r)), '{"step": "fifty op'].join("\n");

test("a line cut short is skipped and every whole one is read", () => {
  assert.equal(readSamples(jsonl).length, run.length);
});

test("a median is the middle sample, or the mean of the two middle ones", () => {
  assert.equal(median([3, 1, 2]), 2);
  assert.equal(median([4, 1, 3, 2]), 2.5);
});

test("on Linux, M2 is the web content's resident memory at fifty open", () => {
  const { entries } = benchmarks(readSamples(jsonl), "ubuntu-24.04");
  const m2 = entries.find((e) => e.name.startsWith("M2"));
  // 300,000, 310,000 and 320,000 KB: the median is 310,000 KB, 302.7 MB.
  assert.equal(m2.value, 302.7);
  assert.equal(m2.unit, "MB");
  assert.match(m2.name, /resident/);
  assert.match(m2.extra, /median of 3 samples/);
});

test("on macOS, M2 is the footprint peak, and a sample footprint could not read is skipped", () => {
  const { entries } = benchmarks(readSamples(jsonl), "macos-latest");
  const m2 = entries.find((e) => e.name.startsWith("M2"));
  // 1,000 MB and 2,000 MB; the third look had no reading and is not read as zero.
  assert.equal(m2.value, 1500);
  assert.match(m2.name, /footprint peak/);
  assert.match(m2.extra, /median of 2 samples/);
});

test("the two systems' M2 are named apart, so they are never one series", () => {
  const name = (os) =>
    benchmarks(readSamples(jsonl), os).entries.find((e) =>
      e.name.startsWith("M2"),
    ).name;
  assert.notEqual(name("macos-latest"), name("ubuntu-24.04"));
});

test("M3 is the app's resident memory in every look with no chat running", () => {
  const { entries } = benchmarks(readSamples(jsonl), "ubuntu-24.04");
  const m3 = entries.find((e) => e.name.startsWith("M3"));
  // 190,000, 330,000, 334,000 and 336,000 KB: the median is 332,000 KB, 324.2 MB.
  assert.equal(m3.value, 324.2);
  assert.match(m3.extra, /median of 4 samples/);
});

test("a row with fewer than two samples is left out with a warning, never recorded from one", () => {
  const one = [
    look("nothing open", { rssKb: 190_000 }),
    look("fifty open", { harnesses: 50, rssKb: 400_000, webContentKb: 300_000 }),
  ];
  const { entries, warnings } = benchmarks(one, "ubuntu-24.04");
  assert.deepEqual(entries, []);
  assert.equal(warnings.length, 2);
  assert.match(warnings[0], /M2.*1 sample/);
});

test("a run with no row writes no file, so the job skips the tool", () => {
  const dir = mkdtempSync(join(tmpdir(), "stress-bench-"));
  const script = fileURLToPath(new URL("./stress-bench.mjs", import.meta.url));
  const out = join(dir, "bench.json");

  const empty = spawnSync(process.execPath, [script, join(dir, "missing.jsonl"), "ubuntu-24.04", out], {
    encoding: "utf8",
  });
  assert.equal(empty.status, 0, empty.stderr);
  assert.equal(existsSync(out), false);
  assert.match(empty.stdout, /::warning title=stress record::M2/);

  const input = join(dir, "stress.jsonl");
  writeFileSync(input, jsonl);
  const written = spawnSync(process.execPath, [script, input, "ubuntu-24.04", out], {
    encoding: "utf8",
  });
  assert.equal(written.status, 0, written.stderr);
  const entries = JSON.parse(readFileSync(out, "utf8"));
  assert.deepEqual(
    entries.map((e) => e.name),
    [
      "M2 web content at fifty hot chats (resident)",
      "M3 native side with no chats (resident)",
    ],
  );
});

/** `stress.yml`'s jobs, each as its own lines, by name: the file's two-space keys under `jobs:`. */
function jobsOf(text) {
  const jobs = {};
  let current = null;
  let inJobs = false;
  for (const line of text.split("\n")) {
    if (/^\S/.test(line)) {
      inJobs = line.startsWith("jobs:");
      current = null;
      continue;
    }
    const key = inJobs && line.match(/^ {2}([\w-]+):\s*$/);
    if (key) {
      current = key[1];
      jobs[current] = [];
    } else if (current) {
      jobs[current].push(line);
    }
  }
  return jobs;
}

test("only the record job can write, and only on main, never in a pull request's run", () => {
  const workflow = fileURLToPath(new URL("../.github/workflows/stress.yml", import.meta.url));
  const jobs = jobsOf(readFileSync(workflow, "utf8"));
  const writers = Object.entries(jobs)
    .filter(([, lines]) => lines.some((l) => /^\s+contents:\s*write\b/.test(l)))
    .map(([name]) => name);
  assert.deepEqual(writers, ["stress-record"]);
  const when = jobs["stress-record"].find((l) => /^ {4}if:/.test(l));
  assert.match(when, /github\.ref == 'refs\/heads\/main'/);
  assert.match(when, /github\.event_name != 'pull_request'/);
  // What it records comes from `fifty tabs`, which no pull request runs (V70).
  assert.ok(jobs["stress-record"].some((l) => /^ {4}needs: stress\s*$/.test(l)));
  // The job that writes restores no cache another job saved.
  const code = jobs["stress-record"].filter((l) => !/^\s*#/.test(l));
  assert.ok(!code.some((l) => /rust-cache|actions\/cache|cache:/.test(l)));
});
