// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The Linux release-profile job (M74.1, T1.1-54): every Linux entry of the switch and
// substitution list gets a check, and the checks go red on a bundle that is a test build.
import assert from "node:assert/strict";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { jobsOf } from "./ci-push-jobs.mjs";
import { CHECKS, COVER, SWITCHES, checkOf, entries, runAll } from "./release-profile-checks.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));

/** A `target/release` whose programs are scripts saying `version`, and `builtWith` to `_built-with`. */
function bundle({ builtWith, version = "purlis 0.5.1 (release profile)", deb = true }) {
  const target = mkdtempSync(join(tmpdir(), "bundle-"));
  const dir = join(target, "release");
  mkdirSync(dir, { recursive: true });
  for (const name of ["purlis", "purlis-app"]) {
    const script = `#!/bin/sh\ncase "$1" in\n  _built-with) ${builtWith ? `echo "${builtWith}"; exit 0` : "echo 'unknown command' >&2; exit 2"} ;;\n  --version) echo "${version}" ;;\nesac\n`;
    writeFileSync(join(dir, name), script);
    chmodSync(join(dir, name), 0o755);
  }
  if (deb) mkdirSync(join(dir, "bundle", "deb"), { recursive: true });
  return target;
}

test("the_linux_release_profile_job_checks_every_linux_switch_entry", () => {
  const list = JSON.parse(readFileSync(SWITCHES, "utf8"));
  const wanted = [
    ...list.switches.filter((s) => (s.covers?.linux ?? []).includes(COVER)),
    ...list.substitutions.filter((s) => (s.covers?.linux ?? []).includes(COVER)),
  ];
  const got = entries(list);
  assert.equal(got.length, wanted.length, "one check per Linux-applicable entry, none dropped");
  assert.ok(got.length > 100, "the list is read");
  for (const e of got) assert.ok(e.check in CHECKS, `${e.id} has a check: ${e.check}`);
  assert.equal(new Set(got.map((e) => e.id)).size, got.length, "an entry's id names it alone");
  // The kinds of difference each get the check that proves them.
  assert.equal(checkOf({ at: "if cfg!(debug_assertions) {" }), "profile");
  assert.equal(checkOf({ at: "if purlis_core::fence::FENCED {" }), "fence");
  // A list entry this job does not cover on Linux is not here.
  const fewer = entries({ switches: [{ file: "a.rs", at: "x", covers: { linux: ["host-process"] } }], substitutions: [] });
  assert.deepEqual(fewer, []);
});

test("the_fence_check_is_red_on_a_bundle_built_with_the_fence", () => {
  const green = bundle({});
  const red = bundle({ builtWith: "fenced=true test-support=false" });
  try {
    assert.equal(CHECKS.fence({ target: green, profile: "release" }), null);
    assert.match(CHECKS.fence({ target: red, profile: "release" }), /answers _built-with/);
    assert.equal(CHECKS.profile({ target: green, profile: "release" }), null);
    const debug = bundle({ version: "purlis 0.5.1 (debug profile)" });
    assert.match(CHECKS.profile({ target: debug, profile: "release" }), /not a release profile build/);
    rmSync(debug, { recursive: true });
    // Every entry the fence gates is red together on the red bundle.
    const ran = runAll({ target: red, profile: "release" });
    const fence = ran.results.filter((r) => r.check === "fence");
    assert.ok(fence.length > 100 && fence.every((r) => r.problem), "each fence entry is red");
    assert.match(CHECKS.bundle({ target: green, profile: "release" }), /no \.deb/);
  } finally {
    rmSync(green, { recursive: true });
    rmSync(red, { recursive: true });
  }
});

test("the_release_profile_job_runs_nightly_and_on_pull_requests_that_change_an_entry", async () => {
  const workflow = readFileSync(join(root, ".github/workflows/sandbox-scenarios.yml"), "utf8");
  const job = jobsOf(workflow).find((j) => j.id === "linux-release-profile");
  assert.ok(job, "sandbox-scenarios.yml has the job linux-release-profile");
  assert.match(job.text, /uses: \.\/\.github\/actions\/linux-release-profile/);
  assert.match(job.text, /needs\.plan\.outputs\.release-profile == 'true'/);
  // The action builds the bundle as `release.yml` does, with the same profile, and runs the checks.
  const action = readFileSync(join(root, ".github/actions/linux-release-profile/action.yml"), "utf8");
  assert.match(action, /release-profile-checks\.mjs run target release/);
  assert.match(action, /npx tauri build .*--bundles deb -- --profile release/);
  assert.match(action, /cargo build --profile release --locked -p purlis-cli -p persona-statistics/);
  // A pull request that changes an entry's file runs it; one far from every entry does not; main's
  // merge queue never does (the entries' own pull requests ran it).
  const { entryFiles, plan: planOf } = await import("./sandbox-scenarios-plan.mjs");
  const entries = entryFiles();
  const pr = (paths) => planOf({ event: "pull_request", paths, entries });
  assert.equal(pr(["app/src-tauri/src/agentidentity.rs"]).releaseProfile, true);
  assert.equal(pr(["app/src-tauri/src/scenarios/switches.json"]).releaseProfile, true);
  assert.equal(pr(["tools/land.mjs"]).releaseProfile, false);
  assert.equal(planOf({ event: "merge_group", entries }).releaseProfile, false);
  const plan = readFileSync(join(root, "tools/sandbox-scenarios-plan.mjs"), "utf8");
  assert.match(plan, /releaseProfile/);
  const list = readFileSync(join(root, ".github/nightly-on-phase-1-proven.txt"), "utf8");
  assert.match(list, /^linux-release-profile\s+ubuntu-24\.04\s+\d+/m);
  const nightly = readFileSync(join(root, ".github/actions/nightly/action.yml"), "utf8");
  assert.match(nightly, /"linux-release-profile"/);
});
