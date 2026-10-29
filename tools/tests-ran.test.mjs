// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// A step that picks tests by name fails when a name picks none (#3752). Run with
// `node --test tools/`.
//
// The script runs a stand-in for `cargo test` that prints what libtest would have printed, so
// no test binary is built.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(dirname(fileURLToPath(import.meta.url)), "tests-ran.sh");

// Runs the script around a stand-in `cargo` that prints `printed` and exits `status`, with the
// arguments after `cargo` as given.
function run(args, printed, status = 0) {
  const root = mkdtempSync(join(tmpdir(), "tests-ran-"));
  const out = join(root, "printed");
  writeFileSync(out, printed);
  const cargo = join(root, "cargo");
  writeFileSync(cargo, `#!/bin/sh\ncat "${out}"\nexit ${status}\n`);
  chmodSync(cargo, 0o755);
  const result = spawnSync("bash", [script, "--", cargo, ...args], {
    encoding: "utf8",
    env: { ...process.env, GITHUB_ACTIONS: "" },
  });
  return { status: result.status, stdout: result.stdout, stderr: result.stderr };
}

const libtest = (lines, passed, failed = 0, ignored = 0) =>
  `\nrunning ${lines.length} tests\n${lines.join("\n")}\n\n` +
  `test result: ${failed ? "FAILED" : "ok"}. ${passed} passed; ${failed} failed; ${ignored} ignored; 0 measured; 1990 filtered out; finished in 1.20s\n\n`;

test("a name that picked its tests passes, and what the tests printed is passed on", () => {
  const printed = libtest(["test guard_a::one ... ok", "test guard_a::two ... ok"], 2);
  const run1 = run(["test", "-p", "purlis-core", "--test", "it", "--", "guard_a::"], printed);
  assert.equal(run1.status, 0, run1.stderr);
  assert.ok(run1.stdout.includes("test guard_a::one ... ok"));
});

test("a name that picked no test fails, and says which", () => {
  const printed = libtest([], 0);
  const r = run(["test", "-p", "purlis-core", "--test", "it", "--", "renamed_away::"], printed);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /renamed_away::/);
});

test("one of several names that picked no test fails, and only it is named", () => {
  const printed = libtest(["test guard_a::one ... ok"], 1);
  const r = run(
    ["test", "-p", "purlis-core", "--test", "it", "--", "guard_a::", "guard_b::"],
    printed,
  );
  assert.equal(r.status, 1);
  assert.match(r.stderr, /guard_b::/);
  assert.doesNotMatch(r.stderr, /guard_a::/);
});

test("a name that picked only ignored tests ran nothing, and fails", () => {
  const printed = libtest(["test live::one ... ignored"], 0, 0, 1);
  const r = run(["test", "-p", "purlis-core", "--test", "it", "--", "live::"], printed);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /live::/);
});

test("with no names, a run of no test at all fails", () => {
  const r = run(["test", "-p", "studio-probe"], libtest([], 0));
  assert.equal(r.status, 1);
});

test("with no names, a run of some tests passes", () => {
  const r = run(["test", "-p", "studio-probe"], libtest(["test a ... ok"], 1));
  assert.equal(r.status, 0, r.stderr);
});

test("a flag's value is not a name", () => {
  const printed = libtest(["test guard_a::one ... ok"], 1);
  const r = run(
    [
      "test", "-p", "purlis-core", "--test", "it", "--",
      "--test-threads", "1", "--skip", "slow", "guard_a::", "--ignored", "--nocapture",
    ],
    printed,
  );
  assert.equal(r.status, 0, r.stderr);
});

test("a test's own output between its name and its verdict still counts it", () => {
  // `--nocapture`: libtest prints the name, then whatever the test printed, then the verdict.
  const printed = libtest(["test search::large ... measured 12 ms", "ok"], 1);
  const r = run(["test", "--", "search::", "--nocapture"], printed);
  assert.equal(r.status, 0, r.stderr);
});

test("a failed command keeps its own status", () => {
  const printed = libtest(["test guard_a::one ... FAILED"], 0, 1);
  const r = run(["test", "--", "guard_a::"], printed, 101);
  assert.equal(r.status, 101);
});
