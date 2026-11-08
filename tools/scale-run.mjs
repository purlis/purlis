#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The nightly scale run's measurements of the `purlis` binary (#5497): a full build of the
// project index, the SessionStart hook, `recall` in one workspace and across all of them, and one
// git standing look after the prime auto-save runs as the app opens the project (timed as a row of
// its own), each on a project `scale-project generate` wrote and the run committed. One
// row a measurement, as `tools/scale-report.mjs` reads them:
//
//   node tools/scale-run.mjs --project <dir> --os macos|linux --out rows.jsonl \
//     [--purlis target/release/purlis] [--scale-project target/release/scale-project] [--times 20]
//
// **The highest seam**: the real binary, started as a harness starts a hook, with the payload
// on its standard input, timed from the spawn to its exit. Each process gets an environment of
// the run's own (a home, a config home and a data home in a temporary folder), never the one it
// was started from, so a chat's socket or persona from the shell around it reaches nothing.
// The first run of each is a warm-up and is not kept: the page cache is what a person's second
// chat of the day meets.
//
// **The index is built first** (#5486), as the session host builds it when it opens the project:
// a person's chats meet a project the host has indexed. Timed before it, the hook and `recall`
// read every file instead (the CLI never writes an index), which measured the scan the index
// replaces, not purlis: the hook took 25 s on macOS for a `git status` the index answers. A command that fails, or a hook that answers no briefing, fails the run:
// a timing of a broken answer is no evidence.
//
// **The index is built on a settled disk** (#5672). Right after `scale-project generate` writes
// its 217,000 files and the run commits them, the runner's disk is still writing them out, and a
// build timed then waited on it: 42 to 95 s on macOS runners that build in 19 to 31 s once quiet.
// So the row waits first: `sync`, then until the disk has moved less than `--quiet-mb` a second
// for `--quiet-for` seconds in a row, at most `--settle-cap` seconds. A person's host meets a
// project already on disk, not one being written. How long it waited, and whether it went quiet,
// is on the row.

import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import process from "node:process";
import { parseArgs } from "node:util";

const { values } = parseArgs({
  options: {
    project: { type: "string" },
    os: { type: "string" },
    out: { type: "string" },
    purlis: { type: "string", default: "target/release/purlis" },
    "scale-project": { type: "string", default: "target/release/scale-project" },
    times: { type: "string", default: "20" },
    workspace: { type: "string", default: "ws-0001" },
    persona: { type: "string", default: "persona-001" },
    query: { type: "string", default: "migration" },
    "quiet-mb": { type: "string", default: "1" },
    "quiet-for": { type: "string", default: "5" },
    "settle-cap": { type: "string", default: "300" },
  },
});
for (const name of ["project", "os", "out"]) {
  if (!values[name]) throw new Error(`--${name} is needed`);
}
const project = resolve(values.project);
const purlis = resolve(values.purlis);
const times = Number(values.times);
const inWorkspace = join(project, "workspaces", values.workspace);

const home = mkdtempSync(join(tmpdir(), "scale-run-"));
const env = {
  PATH: process.env.PATH,
  HOME: home,
  TMPDIR: process.env.TMPDIR ?? tmpdir(),
  LANG: "C.UTF-8",
  CHARTER_ROOT: project,
  CHARTER_CONFIG_HOME: join(home, "config"),
  CHARTER_DATA_HOME: join(home, "data"),
  PURLIS_PERSONA: values.persona,
};

/** Runs `argv` in `cwd` with `input` on its stdin, and answers how long it took, in ms. */
function timed(argv, { cwd = inWorkspace, input = "" } = {}) {
  const started = process.hrtime.bigint();
  const ran = spawnSync(argv[0], argv.slice(1), { cwd, env, input, encoding: "utf8", maxBuffer: 64 << 20 });
  const ms = Number(process.hrtime.bigint() - started) / 1e6;
  if (ran.error) throw ran.error;
  if (ran.status !== 0) {
    throw new Error(`${argv.join(" ")} exited ${ran.status}: ${(ran.stderr || ran.stdout).slice(0, 2000)}`);
  }
  return { ms, stdout: ran.stdout };
}

/** `times` timings of `argv` after one warm-up, each answer checked by `check`. */
function samples(argv, { input, check }) {
  const kept = [];
  for (let n = 0; n <= times; n++) {
    const { ms, stdout } = timed(argv, { input: typeof input === "function" ? input(n) : input });
    check(stdout);
    if (n > 0) kept.push(Math.round(ms * 10) / 10);
  }
  return kept;
}

const rows = [];
const row = (budget, measured) => {
  rows.push({ budget, os: values.os, ...measured });
  console.log(`scale-run | ${budget} | ${JSON.stringify(measured)}`);
};

/**
 * Bytes this machine's disks have read and written since boot, or `undefined` where it cannot
 * tell. Linux sums `/proc/diskstats` over the whole disks in `/sys/block` (a partition's count is
 * in its disk's already); macOS sums the megabytes column of `iostat -Id`.
 */
function diskBytes() {
  if (process.platform === "linux") {
    let sectors = 0;
    for (const line of readFileSync("/proc/diskstats", "utf8").split("\n")) {
      const f = line.trim().split(/\s+/);
      if (f.length < 10 || /^(loop|ram|zram)/.test(f[2]) || !existsSync(`/sys/block/${f[2]}`)) continue;
      sectors += Number(f[5]) + Number(f[9]);
    }
    return sectors * 512;
  }
  if (process.platform === "darwin") {
    const ran = spawnSync("iostat", ["-Id", "-c", "1"], { encoding: "utf8" });
    const last = ran.stdout?.trim().split("\n").at(-1)?.trim().split(/\s+/).map(Number) ?? [];
    let mb = 0;
    for (let i = 2; i < last.length; i += 3) mb += last[i];
    return Number.isFinite(mb) && last.length >= 3 ? mb * 1024 * 1024 : undefined;
  }
  return undefined;
}

/** `sync`, then waits until the disks are quiet, at most `--settle-cap` seconds. */
function settle() {
  const started = Date.now();
  spawnSync("sync");
  const synced = Date.now() - started;
  const quietBytes = Number(values["quiet-mb"]) * 1024 * 1024;
  const quietFor = Number(values["quiet-for"]);
  const cap = started + Number(values["settle-cap"]) * 1000;
  const rates = [];
  let quiet = 0;
  let before = diskBytes();
  if (before === undefined) return { settled_ms: synced, synced_ms: synced, quiet: false, mb_per_s: [] };
  while (quiet < quietFor && Date.now() < cap) {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1000);
    const now = diskBytes();
    const moved = now - before;
    before = now;
    rates.push(Math.round((moved / 1024 / 1024) * 10) / 10);
    quiet = moved < quietBytes ? quiet + 1 : 0;
  }
  return { settled_ms: Date.now() - started, synced_ms: synced, quiet: quiet >= quietFor, mb_per_s: rates };
}

const settled = settle();
console.log(`scale-run | settle | ${JSON.stringify(settled)}`);

// A full build, as `scale-project index` takes it on a project with none: the host's first.
const indexed = timed([resolve(values["scale-project"]), "index", "--project", project], { cwd: project });
row("index-rebuild", {
  samples: [Math.round(JSON.parse(indexed.stdout).built_ms * 10) / 10],
  settled_ms: settled.settled_ms,
  quiet: settled.quiet,
});

// DIAGNOSTIC (#5672), to be reverted: does reading on more threads than cores help?
{
  const { rmSync } = await import("node:fs");
  for (const threads of ["3", "6", "12", "24", "3", "12"]) {
    const ran = spawnSync("/usr/bin/time", [resolve(values["scale-project"]), "readbench", "--project", project, "--mode", "open+read"], { cwd: project, env: { ...env, SCRATCH_THREADS: threads }, encoding: "utf8" });
    console.log(`scale-run | diag read | ${ran.stdout.trim()} | ${ran.stderr.replace(/\n\s*/g, " ").trim()}`);
  }
  for (const threads of ["3", "6", "12", "3", "6", "12"]) {
    rmSync(join(project, ".charter", "index"), { recursive: true, force: true });
    const ran = spawnSync("/usr/bin/time", [resolve(values["scale-project"]), "index", "--project", project], { cwd: project, env: { ...env, SCRATCH_THREADS: threads }, encoding: "utf8" });
    console.log(`scale-run | diag index ${threads} threads | ${ran.stdout.trim()} | ${ran.stderr.replace(/\n\s*/g, " ").trim().slice(0, 200)}`);
  }
}

row("hook", {
  samples: samples([purlis, "hook", "sessionstart"], {
    input: (n) => JSON.stringify({ session_id: `scale-${n}`, hook_event_name: "SessionStart", cwd: inWorkspace }),
    check(stdout) {
      const context = JSON.parse(stdout.trim()).hookSpecificOutput?.additionalContext ?? "";
      if (!context.includes(values.persona)) throw new Error(`the hook briefed no persona: ${stdout.slice(0, 500)}`);
    },
  }),
});

const found = (stdout) => {
  if (!stdout.includes(values.query)) throw new Error(`recall found nothing for ${values.query}: ${stdout.slice(0, 500)}`);
};
row("recall", {
  samples: samples([purlis, "recall", values.query, "-w", values.workspace, "--persona", values.persona], {
    check: found,
  }),
});
row("recall-all", {
  samples: samples([purlis, "recall", values.query, "--all-workspaces", "--persona", values.persona], {
    check: found,
  }),
});

const look = timed([resolve(values["scale-project"]), "look", "--project", project, "--times", "5"], { cwd: project });
const looked = JSON.parse(look.stdout);
row("git-prime", { samples: [Math.round(looked.primed_ms * 10) / 10] });
row("git-standing", { samples: looked.samples_ms.map((ms) => Math.round(ms * 10) / 10) });

writeFileSync(values.out, "");
for (const r of rows) appendFileSync(values.out, `${JSON.stringify(r)}\n`);
