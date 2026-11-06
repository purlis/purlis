#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The nightly scale run's measurements of the `purlis` binary (#5497): the SessionStart hook,
// `recall` in one workspace and across all of them, and one git standing look, each on a
// project `scale-project generate` wrote and the run committed. One row a measurement, as
// `tools/scale-report.mjs` reads them:
//
//   node tools/scale-run.mjs --project <dir> --os macos|linux --out rows.jsonl \
//     [--purlis target/release/purlis] [--scale-project target/release/scale-project] [--times 20]
//
// **The highest seam**: the real binary, started as a harness starts a hook, with the payload
// on its standard input, timed from the spawn to its exit. Each process gets an environment of
// the run's own (a home, a config home and a data home in a temporary folder), never the one it
// was started from, so a chat's socket or persona from the shell around it reaches nothing.
// The first run of each is a warm-up and is not kept: the page cache is what a person's second
// chat of the day meets. A command that fails, or a hook that answers no briefing, fails the run:
// a timing of a broken answer is no evidence.

import { spawnSync } from "node:child_process";
import { appendFileSync, mkdtempSync, writeFileSync } from "node:fs";
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
row("git-standing", { samples: JSON.parse(look.stdout).samples_ms.map((ms) => Math.round(ms * 10) / 10) });

writeFileSync(values.out, "");
for (const r of rows) appendFileSync(values.out, `${JSON.stringify(r)}\n`);
