// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What `tools/coldstart-linux.sh` does when a cold start cannot be TAKEN (#1876): the cause and
// the case on one annotation, a passing step where the gate only reports, a failing one where it
// does not. The X server, i3 and the bench are stand-ins on PATH; the bench itself is run for
// real against stand-in apps, where the machine can (it writes a `charter.toml`, and on macOS
// asks the system to bring a window forward). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { platform, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";

const TOOLS = import.meta.dirname;
const SCRIPT = join(TOOLS, "coldstart-linux.sh");

function program(dir, name, body) {
  const path = join(dir, name);
  writeFileSync(path, `#!/bin/sh\n${body}\n`);
  chmodSync(path, 0o755);
  return path;
}

/** Stand-ins for everything the script starts, and the script run with them first on PATH. */
function script(args, { env = {}, display = ":77" } = {}) {
  const dir = mkdtempSync(join(tmpdir(), "coldstart-linux-test-"));
  // `xvfb-run [-a] [-e file] [-s args] command…`: the display it would have made, or its failure.
  program(
    dir,
    "xvfb-run",
    `if [ "$STUB_X" = fails ]; then
  while [ $# -gt 0 ]; do
    if [ "$1" = -e ]; then echo "Xvfb: the display is already taken" >"$2"; fi
    shift
  done
  echo "xvfb-run: error: Xvfb failed to start" >&2
  exit 1
fi
while [ $# -gt 0 ]; do
  case "$1" in
  -a) shift ;;
  -e | -s) shift 2 ;;
  *) break ;;
  esac
done
DISPLAY=:78 exec "$@"`,
  );
  // `i3 -c config` is the window manager; `i3 --get-socketpath` succeeds once one manages.
  program(
    dir,
    "i3",
    `if [ "$1" = --get-socketpath ]; then
  [ -e "${dir}/i3-up" ] && echo "${dir}/i3-up" && exit 0
  exit 1
fi
echo start >>"${dir}/i3-starts"
case "$STUB_I3" in
leaves)
  echo "i3: cannot open the display" >&2
  exit 1
  ;;
second)
  if [ "$(wc -l <"${dir}/i3-starts")" -lt 2 ]; then
    echo "i3: cannot open the display" >&2
    exit 1
  fi
  ;;
esac
touch "${dir}/i3-up"
exec sleep 30`,
  );
  // The bench: what it was run with, and the status the test asks of it.
  program(dir, "node", `echo "$@" >"${dir}/bench-args"\nexit "\${STUB_BENCH:-0}"`);
  // The script is Linux's. Elsewhere (macOS), GNU `realpath -m`, and a `mktemp -d` that keeps
  // to the temp directory this test was given.
  if (platform() !== "linux") {
    program(dir, "realpath", `for last; do :; done\necho "$last"`);
    program(dir, "mktemp", `made="${dir}/tmp.$$"\nmkdir "$made" && echo "$made"`);
  }
  const app = program(dir, "app", "exit 0");
  const summary = join(dir, "summary.md");
  const withEnv = { ...process.env, APP: app, ...env };
  for (const name of ["DISPLAY", "EVIDENCE_ONLY", "BASELINE_APP", "GITHUB_STEP_SUMMARY"])
    if (!(name in env)) delete withEnv[name];
  if (display) withEnv.DISPLAY = display;
  const done = spawnSync("bash", [SCRIPT, ...args], {
    encoding: "utf8",
    env: {
      ...withEnv,
      PATH: `${dir}:${process.env.PATH}`,
      GITHUB_STEP_SUMMARY: summary,
      I3_RESTART_PAUSE: "0",
    },
  });
  const read = (name) => (existsSync(join(dir, name)) ? readFileSync(join(dir, name), "utf8") : "");
  const result = {
    status: done.status,
    out: done.stdout,
    err: done.stderr,
    summary: read("summary.md"),
    benchArgs: read("bench-args"),
    i3Starts: read("i3-starts").trim().split("\n").filter(Boolean).length,
  };
  rmSync(dir, { recursive: true, force: true });
  return result;
}

test("a window manager that never came up is a warning and a passing step where the gate only reports", () => {
  const run = script(["i3"], { env: { EVIDENCE_ONLY: "1", STUB_I3: "leaves" } });
  assert.equal(run.status, 0, run.err);
  assert.match(run.out, /^::warning title=Cold start%2C i3%3A no measurement::i3 did not come up in 3 starts/m);
  assert.match(run.out, /evidence only/);
  // i3's own words are on the annotation, so the cause is read without the log.
  assert.match(run.out, /i3%3A cannot open the display|i3: cannot open the display/);
  assert.match(run.summary, /Cold start, i3: no measurement/);
  assert.equal(run.i3Starts, 3);
  assert.equal(run.benchArgs, "", "nothing is measured on a desktop that is not the case's");
});

test("a window manager that never came up fails the step where the gate decides", () => {
  const run = script(["i3"], { env: { STUB_I3: "leaves" } });
  assert.equal(run.status, 1);
  assert.match(run.out, /^::error title=Cold start%2C i3%3A no measurement::i3 did not come up/m);
  assert.doesNotMatch(run.out, /evidence only/);
  assert.match(run.summary, /no measurement/);
});

test("the no-bus case says its own name when i3 never came up", () => {
  const run = script(["i3-nobus"], { env: { EVIDENCE_ONLY: "1", STUB_I3: "leaves" } });
  assert.equal(run.status, 0, run.err);
  assert.match(run.out, /^::warning title=Cold start%2C i3-nobus%3A no measurement::/m);
});

test("i3 leaving at once is started again, measured, and said", () => {
  const run = script(["i3"], { env: { EVIDENCE_ONLY: "1", STUB_I3: "second" } });
  assert.equal(run.status, 0, run.err);
  assert.equal(run.i3Starts, 2);
  assert.match(run.out, /^::warning title=Cold start%2C i3%3A i3 came up on start 2::/m);
  assert.match(run.benchArgs, /tools\/bench\.mjs .*--case i3 .*--evidence/);
  assert.equal(run.summary, "");
});

test("i3 up at the first start measures and says nothing more", () => {
  const run = script(["i3"], { env: { EVIDENCE_ONLY: "1" } });
  assert.equal(run.status, 0, run.err);
  assert.equal(run.i3Starts, 1);
  assert.doesNotMatch(run.out, /^::/m);
  assert.match(run.benchArgs, /--case i3 /);
});

test("an X server that never came up is a warning and a passing step where the gate only reports", () => {
  const run = script(["bare"], { display: "", env: { EVIDENCE_ONLY: "1", STUB_X: "fails" } });
  assert.equal(run.status, 0, run.err);
  assert.match(
    run.out,
    /^::warning title=Cold start%2C bare%3A no measurement::the X server did not come up: xvfb-run ended with 1/m,
  );
  assert.match(run.out, /the display is already taken/);
  assert.match(run.summary, /Cold start, bare: no measurement/);
  assert.equal(run.benchArgs, "");
});

test("an X server that never came up fails the step where the gate decides", () => {
  const run = script(["bare"], { display: "", env: { STUB_X: "fails" } });
  assert.equal(run.status, 1);
  assert.match(run.out, /^::error title=Cold start%2C bare%3A no measurement::the X server did not come up/m);
});

test("the bench's own verdict inside the X server is the step's, softened by nothing here", () => {
  const failed = script(["bare"], { display: "", env: { EVIDENCE_ONLY: "1", STUB_BENCH: "1" } });
  assert.equal(failed.status, 1);
  assert.doesNotMatch(failed.out, /no measurement/);
  const passed = script(["i3"], { display: "", env: { EVIDENCE_ONLY: "1" } });
  assert.equal(passed.status, 0, passed.err);
  assert.match(passed.benchArgs, /--case i3 /);
});

test("a build that is not where the job put it fails, report-only or not", () => {
  const run = script(["bare"], { env: { EVIDENCE_ONLY: "1", APP: "/nowhere/charter-app" } });
  assert.equal(run.status, 1);
  assert.match(run.out, /^::error title=Cold start%2C bare%3A the script could not run::the app to launch is not at/m);
  const main = script(["bare"], { env: { EVIDENCE_ONLY: "1", BASELINE_APP: "/nowhere/main" } });
  assert.equal(main.status, 1);
  assert.match(main.out, /main's app is not at/);
});

test("an unknown case fails, report-only or not", () => {
  const run = script(["gnome"], { env: { EVIDENCE_ONLY: "1" } });
  assert.equal(run.status, 2);
  assert.match(run.out, /^::error title=Cold start%2C gnome%3A the script could not run::unknown case: gnome/m);
  assert.equal(script([], { env: { EVIDENCE_ONLY: "1" } }).status, 2);
});

// The bench for real, against apps that are a line of shell. Linux only: it writes a
// `charter.toml` into the temp directory and, on macOS, asks the system for the window.
const bench = { skip: platform() !== "linux" && "the bench is run against stand-in apps on Linux (CI)" };

function benchRun(apps, flags = []) {
  const dir = mkdtempSync(join(tmpdir(), "coldstart-bench-test-"));
  const kinds = {
    draws: 'echo "charter-bench first-frame 12"\nexec sleep 30',
    never: "exec sleep 30",
    leaves: 'echo "Gtk-WARNING: cannot open display" >&2\nexit 3',
  };
  const app = program(dir, "change", kinds[apps.change]);
  const main = apps.main ? program(dir, "main", kinds[apps.main]) : undefined;
  const summary = join(dir, "summary.md");
  const done = spawnSync(
    process.execPath,
    [
      resolve(TOOLS, "bench.mjs"),
      ...["--skip-build", "--only", "coldstart", "--app", app, "--limit", "2000", "--ceiling", "2500"],
      ...["--warm-up", "--fresh-profile", "--case", "i3", "--cold-starts", "1"],
      ...["--first-frame-timeout", "400"],
      ...(main ? ["--baseline-app", main] : []),
      ...flags,
    ],
    { encoding: "utf8", env: { ...process.env, GITHUB_ACTIONS: "true", GITHUB_STEP_SUMMARY: summary } },
  );
  const result = {
    status: done.status,
    out: done.stdout,
    err: done.stderr,
    summary: existsSync(summary) ? readFileSync(summary, "utf8") : "",
  };
  rmSync(dir, { recursive: true, force: true });
  return result;
}

test("a window that never appeared is a warning and a passing run where the gate only reports", bench, () => {
  const run = benchRun({ change: "never" }, ["--evidence"]);
  assert.equal(run.status, 0, run.err);
  assert.match(
    run.out,
    /^::warning title=Cold start%2C i3%3A no measurement::this change's build drew no first frame within 0\.4 s/m,
  );
  assert.match(run.summary, /Cold start, i3: no measurement/);
});

test("a window that never appeared fails the run where the gate decides", bench, () => {
  const run = benchRun({ change: "never" });
  assert.equal(run.status, 1);
  assert.match(run.out, /^::error title=Cold start%2C i3%3A no measurement::this change's build drew no first frame/m);
});

test("main's build leaving before its first frame is the runner's: a warning where the gate only reports", bench, () => {
  const run = benchRun({ change: "draws", main: "leaves" }, ["--evidence"]);
  assert.equal(run.status, 0, run.err);
  assert.match(run.out, /^::warning title=Cold start%2C i3%3A no measurement::main's build exited \(3\) before its first frame/m);
  assert.match(run.out, /cannot open display/);
});

test("this change's build leaving before its first frame fails, report-only or not", bench, () => {
  const run = benchRun({ change: "leaves" }, ["--evidence"]);
  assert.equal(run.status, 1);
  assert.match(run.out, /^::error title=Cold start%2C i3%3A no measurement::this change's build exited \(3\) before its first frame/m);
  assert.doesNotMatch(run.out, /evidence only/);
});

test("a run that draws is measured as before", bench, () => {
  const run = benchRun({ change: "draws", main: "draws" }, ["--evidence"]);
  assert.equal(run.status, 0, run.err);
  assert.doesNotMatch(run.out, /no measurement/);
  assert.match(run.out, /against main: ratio/);
});
