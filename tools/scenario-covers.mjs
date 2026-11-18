#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **Running the `fixture:` and `e2e:` covers** (M74.1, T1.1-54, #6901). The matrix coverage list
// (`tests/scenarios/coverage.json`) names, for each row it leaves to a fixture test or a window
// spec, the test and the OS it runs on. A scenario leg's report says nothing of them, so this runs
// each named test on the OS its cover names and writes its record in ticket 5's schema, for
// `judge_runs` (the run-result gate step) to judge on a pass like any cover.
//
//     node tools/scenario-covers.mjs plan <os>            # `fixtures=<n>` and `e2e=<n>` for this OS
//     node tools/scenario-covers.mjs run <os> [fixture|e2e]   # runs them, writes the records
//
// - `fixture: <test file>::<test fn>`: `cargo nextest run -p <the file's package> --no-tests=fail
//   -E 'test(/(^|::)<fn>$/)'`, `--retries 0`. A test that is `#[ignore]`d or filtered out runs no
//   test, and nextest exits 4 for it: that is `not_run`, never a pass.
// - `e2e: <spec>#<test title>`: the existing WebdriverIO setup (`npm run e2e` in `app/`), with the
//   spec and `--mochaOpts.grep` for the title, so the same app build, harness and window run it.
//
// A test that returns early before it asserts is not caught here: a record says the test passed,
// as the test says. Records go to `target/sandbox-scenarios/<os>-covers.jsonl`, which the run's
// artifacts carry with the scenario legs'. `pending:` covers run nothing; the gate fails those.

import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));

/** `{ kind, row, os, file|spec, test }` for each fixture and e2e cover of `os` in `list`. */
export function coversOf(list, os) {
  const out = [];
  const take = (entry) => {
    const [kind, rest] = String(entry.cover ?? "").split(/: (.*)/s);
    if (entry.os !== os) return;
    if (kind === "fixture") {
      const at = rest.lastIndexOf("::");
      out.push({ kind: "fixture", row: entry.row, os, file: rest.slice(0, at), test: rest.slice(at + 2) });
    } else if (kind === "e2e") {
      const at = rest.indexOf("#");
      out.push({ kind: "e2e", row: entry.row, os, spec: rest.slice(0, at), test: rest.slice(at + 1) });
    }
  };
  for (const entry of list.rows ?? []) take(entry);
  for (const entry of list.fixtures ?? []) take(entry);
  return out;
}

/** The cargo package of `file` (relative to `base`): its nearest `Cargo.toml`'s `name`. */
export function packageOf(file, base = root) {
  let dir = dirname(resolve(base, file));
  while (dir.startsWith(resolve(base))) {
    const manifest = join(dir, "Cargo.toml");
    if (existsSync(manifest)) {
      const name = /^\s*name\s*=\s*"([^"]+)"/m.exec(readFileSync(manifest, "utf8").split(/^\[/m).find((s) => s.startsWith("package]")) ?? "");
      if (name) return name[1];
    }
    dir = dirname(dir);
  }
  throw new Error(`${file} is in no cargo package`);
}

/** The command that runs `cover`: `[program, args, cwd]`. */
export function commandOf(cover, base = root) {
  if (cover.kind === "fixture") {
    return [
      "cargo",
      ["nextest", "run", "--locked", "--profile", "ci", "--retries", "0", "--no-tests=fail", "-p", packageOf(cover.file, base), "-E", `test(/(^|::)${cover.test}$/)`],
      base,
    ];
  }
  return ["npm", ["run", "e2e", "--", "--spec", cover.spec, "--mochaOpts.grep", cover.test], join(base, "app")];
}

/** The leg-report record of `cover` having run with `status` (a process exit code). */
export function recordOf(cover, status) {
  const base = { kind: cover.kind, row: cover.row, test: cover.test, os: cover.os, leg: cover.kind, timings: "scaled" };
  if (status === 0) return { ...base, verdict: "pass" };
  if (cover.kind === "fixture" && status === 4) {
    return { ...base, verdict: "not_run", reason: "no test ran: it is ignored, filtered out or does not exist" };
  }
  return { ...base, verdict: "fail" };
}

/**
 * Whether an e2e cover's test is in its spec (the file exists and names the test). The WebdriverIO
 * run exits 0 when its filter picks no test, so a cover that names a test that is not there would
 * pass: this is the equivalent of the fixtures' `--no-tests=fail`, asked before the run.
 */
export function specHasTest(cover, base = root) {
  const file = [join(base, cover.spec), join(base, "app", cover.spec)].find((f) => existsSync(f));
  return file !== undefined && readFileSync(file, "utf8").includes(cover.test);
}

/** Runs every cover of `os` (of `kind`, when given); returns the records. `exec` and `has` are injectable. */
export function runCovers(list, os, { kind, exec, has = specHasTest, base = root } = {}) {
  const run =
    exec ??
    ((program, args, cwd) => {
      console.log(`::group::${program} ${args.join(" ")}`);
      const done = spawnSync(program, args, { cwd, stdio: "inherit" });
      console.log("::endgroup::");
      return done.status ?? 1;
    });
  return coversOf(list, os)
    .filter((c) => !kind || c.kind === kind)
    .map((cover) => {
      if (cover.kind === "e2e" && !has(cover, base)) {
        return { kind: cover.kind, row: cover.row, test: cover.test, os: cover.os, leg: cover.kind, timings: "scaled", verdict: "not_run", reason: "no test ran: the spec does not name it" };
      }
      const [program, args, cwd] = commandOf(cover, base);
      return recordOf(cover, run(program, args, cwd));
    });
}

function main([command, os, kind]) {
  if (!["linux", "macos"].includes(os)) {
    console.error("usage: scenario-covers.mjs plan|run <linux|macos> [fixture|e2e]");
    return 2;
  }
  const list = JSON.parse(readFileSync(join(root, "tests/scenarios/coverage.json"), "utf8"));
  if (command === "plan") {
    const all = coversOf(list, os);
    const out = process.env.GITHUB_OUTPUT;
    const lines = [`fixtures=${all.filter((c) => c.kind === "fixture").length}`, `e2e=${all.filter((c) => c.kind === "e2e").length}`];
    for (const line of lines) (out ? appendFileSync(out, `${line}\n`) : console.log(line));
    return 0;
  }
  if (command === "run") {
    const records = runCovers(list, os, { kind });
    const file = join(root, "target/sandbox-scenarios", `${os}-covers.jsonl`);
    mkdirSync(dirname(file), { recursive: true });
    for (const r of records) appendFileSync(file, `${JSON.stringify(r)}\n`);
    const red = records.filter((r) => r.verdict !== "pass");
    console.log(`${records.length} ${kind ?? "fixture and e2e"} covers ran on ${os}: ${red.length} not green`);
    for (const r of red) console.log(`::error title=${r.kind} cover::${r.row}: ${r.test}: ${r.verdict}${r.reason ? ` (${r.reason})` : ""}`);
    return red.length === 0 ? 0 : 1;
  }
  console.error("usage: scenario-covers.mjs plan|run <os>");
  return 2;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
