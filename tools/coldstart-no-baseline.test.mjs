// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What CI's step does when main's app did not build in a pull request's job (#2388): it reads
// main's own `app builds (ubuntu-24.04)` on that commit and either says why there is no baseline
// or fails. `gh` is a stand-in on PATH. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const SCRIPT = join(import.meta.dirname, "coldstart-no-baseline.mjs");
const SHA = "0123456789abcdef0123456789abcdef01234567";

/** The step's script, with a `gh` that answers `answer` (or fails), and what it left. */
function step(answer, { ghFails = false } = {}) {
  const dir = mkdtempSync(join(tmpdir(), "coldstart-no-baseline-test-"));
  const gh = join(dir, "gh");
  writeFileSync(join(dir, "answer.json"), JSON.stringify(answer ?? {}));
  writeFileSync(
    gh,
    ghFails
      ? '#!/bin/sh\necho "HTTP 403: Resource not accessible by integration" >&2\nexit 1\n'
      : `#!/bin/sh\necho "$@" >"${dir}/gh-args"\ncat "${dir}/answer.json"\n`,
  );
  chmodSync(gh, 0o755);
  const output = join(dir, "output");
  writeFileSync(output, "");
  const done = spawnSync(process.execPath, [SCRIPT, SHA], {
    encoding: "utf8",
    env: { ...process.env, PATH: `${dir}:${process.env.PATH}`, GITHUB_OUTPUT: output, GITHUB_REPOSITORY: "purlis/purlis" },
  });
  const read = (name) => (existsSync(join(dir, name)) ? readFileSync(join(dir, name), "utf8") : "");
  const result = { status: done.status, out: done.stdout, err: done.stderr, output: read("output"), ghArgs: read("gh-args") };
  rmSync(dir, { recursive: true, force: true });
  return result;
}

test("main failing on main too: no baseline, its reason in the step's output, and the step passes", () => {
  const run = step({ check_runs: [{ status: "completed", conclusion: "failure" }] });
  assert.equal(run.status, 0, run.err);
  assert.match(run.ghArgs, /repos\/purlis\/purlis\/commits\/0123456789abcdef0123456789abcdef01234567\/check-runs/);
  assert.match(run.ghArgs, /check_name=app builds \(ubuntu-24\.04\)/);
  assert.equal(run.output, "no-baseline=main's app did not build in this job, nor on main (0123456789ab: failure)\n");
  assert.match(run.out, /^::warning title=Cold start%3A no baseline::/m);
});

test("main green on main: the step fails and leaves no baseline to go ungated on", () => {
  const run = step({ check_runs: [{ status: "completed", conclusion: "success", html_url: "https://example.test/1" }] });
  assert.equal(run.status, 1);
  assert.equal(run.output, "");
  assert.match(run.out, /^::error title=Cold start%3A main's app did not build in this job::main's app built on main/m);
});

test("main's check that could not be read fails the step", () => {
  const run = step(undefined, { ghFails: true });
  assert.equal(run.status, 1);
  assert.equal(run.output, "");
  assert.match(run.out, /^::error title=.*could not be read: HTTP 403/m);
});
