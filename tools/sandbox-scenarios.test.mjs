// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The sandbox scenarios' CI (M74.1, T1.1-09): what a run plans from its event, labels and paths
// (`sandbox-scenarios-plan.mjs`), and how `.github/workflows/sandbox-scenarios.yml` and its actions
// wire that plan. The test names are the ticket's.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { jobsOf } from "./ci-push-jobs.mjs";
import { plan } from "./sandbox-scenarios-plan.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(root, path), "utf8");
const WORKFLOW = ".github/workflows/sandbox-scenarios.yml";
const job = (id) => {
  const found = jobsOf(read(WORKFLOW)).find((j) => j.id === id);
  assert.ok(found, `${WORKFLOW} has no job ${id}`);
  return found;
};
const pr = (paths, labels = [], extra = {}) => plan({ event: "pull_request", paths, labels, ...extra });
// The code of a file, its comments left out, so a comment never passes a test.
const code = (text) =>
  text
    .split("\n")
    .filter((line) => !/^\s*#/.test(line))
    .join("\n");

test("the_sandbox_scenarios_job_runs_on_a_pr_touching_no_sandbox_file", () => {
  // A change far from the sandbox runs the suite: the deny-list skips documentation alone.
  for (const path of ["app/src/Notice.tsx", "tools/land.mjs", "crates/purlis-terminal/src/lib.rs"]) {
    const ran = pr([path]);
    assert.equal(ran.linux, true, path);
    assert.match(ran.why, /runs/, path);
  }
  // Documentation alone is skipped, and the plan says why.
  const docs = pr(["docs/development.md", "changes/x.md", "CLAUDE.md"]);
  assert.equal(docs.linux, false);
  assert.match(docs.why, /documentation alone/);
  // A doc that code reads is not documentation alone.
  assert.equal(pr(["docs/plane-format.md"]).linux, true);
  assert.equal(pr(["CHANGELOG.md"]).linux, true);
  // A deleted doc may be linked from a file a test holds: it runs.
  assert.equal(pr(["docs/old.md"], [], { deleted: ["docs/old.md"] }).linux, true);

  // The job always runs: no path filter in the workflow, and only `!cancelled()` on the Linux job,
  // which prints the plan's reason for whatever it runs or skips.
  const text = code(read(WORKFLOW));
  assert.doesNotMatch(text, /^\s*paths(-ignore)?:/m);
  const linux = job("sandbox-scenarios-linux");
  assert.equal(linux.name, "sandbox-scenarios-linux");
  assert.equal(linux.if, "${{ !cancelled() }}");
  assert.ok(linux.needs.includes("plan"));
  assert.match(linux.text, /needs\.plan\.outputs\.why/);
  assert.match(linux.text, /needs\.plan\.outputs\.linux != 'false'/);
});

test("a_pr_changing_cargo_lock_runs_the_full_suite", () => {
  for (const path of [
    "Cargo.lock",
    "rust-toolchain.toml",
    "app/src-tauri/plugin/hooks/hooks.json",
    ".github/workflows/ci.yml",
    ".github/actions/nightly/action.yml",
  ]) {
    assert.equal(pr([path]).linux, true, path);
    assert.equal(pr(["docs/development.md", path]).linux, true, `docs and ${path}`);
  }
});

test("the_merge_queue_never_skips", () => {
  for (const event of ["merge_group", "schedule", "workflow_dispatch"]) {
    const queued = plan({ event, paths: ["docs/development.md"], labels: [] });
    assert.equal(queued.linux, true, event);
    assert.equal(queued.macos, true, event);
  }
  // The queue's runs include the production-timing entries (ticket 54 item 8).
  assert.equal(plan({ event: "merge_group", paths: [], labels: [] }).productionTimings, true);
  assert.match(code(read(WORKFLOW)), /^ {2}merge_group:/m);
});

test("a_labelled_pr_into_phase_1_proven_runs_the_macos_scenarios", () => {
  assert.equal(pr(["crates/purlis-core/src/sandbox.rs"], ["macos-scenarios"], { draft: true }).macos, true);
  assert.equal(pr(["crates/purlis-core/src/sandbox.rs"], [], { draft: true }).macos, false);
  assert.equal(pr(["docs/development.md"], ["macos-scenarios"]).macos, false);

  const text = code(read(WORKFLOW));
  // Pull requests into the branch and into main (ticket 55 re-points by settings alone), and a
  // label put on starts a run. Drafts run: nothing in the workflow reads `draft`.
  assert.match(text, /pull_request:\n\s+branches: \[phase-1-proven, main\]/);
  assert.match(text, /types: \[[^\]]*\blabeled\b[^\]]*\]/);
  assert.doesNotMatch(text, /draft/);
  const macos = job("sandbox-scenarios-macos");
  assert.match(macos.if, /needs\.plan\.outputs\.macos != 'false'/);
  // Pinned macOS versions, never `macos-latest`.
  const versions = /os: \[([^\]]+)\]/.exec(macos.text)?.[1].split(",").map((s) => s.trim());
  assert.ok(versions?.length >= 1, "a matrix of macOS versions");
  for (const os of versions) assert.match(os, /^macos-\d+$/);
});

test("a_production_timings_labelled_pr_runs_the_production_timing_entries", () => {
  const linux = pr(["crates/purlis-core/src/lib.rs"], ["production-timings"]);
  assert.equal(linux.productionTimings, true);
  assert.equal(linux.macos, false);
  const both = pr(["crates/purlis-core/src/lib.rs"], ["production-timings", "macos-scenarios"]);
  assert.equal(both.productionTimings, true);
  assert.equal(both.macos, true);
  assert.equal(pr(["crates/purlis-core/src/lib.rs"]).productionTimings, false);
  // Each job hands the plan's answer to the action that runs the entries.
  for (const id of ["sandbox-scenarios-linux", "sandbox-scenarios-macos"]) {
    assert.match(job(id).text, /production-timings: \$\{\{ needs\.plan\.outputs\.production-timings \}\}/, id);
  }
  // And the action runs the production-timing set when told to.
  const action = code(read(".github/actions/sandbox-scenarios/action.yml"));
  assert.match(action, /inputs\.production-timings == 'true'/);
  assert.match(action, /tools\/sandbox-scenarios\.sh production-timings/);
});

test("a_stress_x20_labelled_pr_runs_its_batches_twenty_times_under_load", () => {
  assert.equal(pr(["crates/purlis-core/src/lib.rs"], ["stress-x20"]).stress, true);
  assert.equal(pr(["crates/purlis-core/src/lib.rs"]).stress, false);
  for (const id of ["sandbox-scenarios-linux", "sandbox-scenarios-macos"]) {
    assert.match(job(id).text, /stress: \$\{\{ needs\.plan\.outputs\.stress \}\}/, id);
  }
  const { calls, status } = runScript("suite", { STRESS: "true" });
  assert.equal(status, 0);
  assert.equal(calls.filter((c) => c.startsWith("cargo ")).length, 20);
  assert.ok(calls.some((c) => c.startsWith("stress-ng ")), "a CPU stress runs beside them");
});

test("the_existing_macos_sandbox_targets_run_on_the_branch_and_fail_on_red", () => {
  const tests = job("macos-sandbox-tests");
  assert.match(tests.if, /needs\.plan\.outputs\.macos != 'false'/);
  assert.equal(tests.continueOnError, undefined, "red fails the job");
  assert.match(tests.text, /uses: \.\/\.github\/actions\/macos-sandbox-tests/);
  assert.match(tests.text, /purlis-app: true/);

  // One copy of the steps: `studio-confinement-macos` runs the same action, under its own scope.
  const confinement = jobsOf(read(".github/workflows/ci.yml")).find((j) => j.id === "studio-confinement-macos");
  assert.match(confinement.text, /uses: \.\/\.github\/actions\/macos-sandbox-tests/);
  assert.doesNotMatch(confinement.text, /cargo test/);

  // The action fails on red: no step of it is let off.
  const action = code(read(".github/actions/macos-sandbox-tests/action.yml"));
  assert.doesNotMatch(action, /continue-on-error/);
  for (const target of [
    "-p studio-probe",
    "-p purlis-core --test it -- a_sandboxed_chat_listens_nowhere_under_seatbelt::",
    "-p purlis-core --lib -- sandbox::localport::tests::under_seatbelt",
    "-p purlis-cli --test it -- a_chats_ssh_route_carries_from_inside_its_sandbox::",
    "-p purlis-core --lib -- secrets::brokered::tests::under_seatbelt",
    "-p purlis-core --lib -- hookwire::serve::tests::",
    "-p purlis-core --lib -- secrets::keyhold::held_by_the_keychain::",
    "-p purlis-app",
  ]) {
    assert.ok(action.includes(target), target);
  }

  // Nightly on the branch too, through the scheduler's list.
  assert.match(read(".github/nightly-on-phase-1-proven.txt"), /^existing-macos-sandbox-tests\s+macos-/m);
});

test("every scenario run has no retries", () => {
  const { calls } = runScript("suite");
  assert.equal(calls.length, 1);
  assert.match(calls[0], /^cargo nextest run .*--retries 0\b/);
  assert.match(runScript("production-timings").calls[0], /--retries 0\b/);
});

test("a red scenario run fails, and a quiet one prints nothing of it", () => {
  const loud = runScript("suite", { FAKE_CARGO_FAILS: "1" });
  assert.notEqual(loud.status, 0);
  assert.match(loud.output, /what an escape would say/);
  const quiet = runScript("suite", { FAKE_CARGO_FAILS: "1", QUIET: "true", ENTRY: "an-entry" });
  assert.notEqual(quiet.status, 0);
  assert.doesNotMatch(quiet.output, /what an escape would say/);
  assert.match(quiet.output, /an-entry/);
  // Red under load stays red, quiet or not, at its first red run.
  for (const QUIET of ["true", "false"]) {
    const stressed = runScript("suite", { FAKE_CARGO_FAILS: "1", STRESS: "true", QUIET });
    assert.notEqual(stressed.status, 0, `quiet ${QUIET}`);
    assert.equal(stressed.calls.filter((c) => c.startsWith("cargo ")).length, 1, `quiet ${QUIET}`);
  }
});

test("an unknown scenario set is refused", () => {
  assert.notEqual(runScript("everything").status, 0);
});

/** `tools/sandbox-scenarios.sh <set>` with `cargo` and `stress-ng` stood in for. */
function runScript(set, env = {}) {
  const dir = mkdtempSync(join(tmpdir(), "sandbox-scenarios-"));
  try {
    const log = join(dir, "calls");
    for (const name of ["cargo", "stress-ng"]) {
      const fake = join(dir, name);
      writeFileSync(
        fake,
        `#!/bin/sh\necho "${name} $*" >> "${log}"\n` +
          (name === "cargo"
            ? `[ -n "$FAKE_CARGO_FAILS" ] && { echo "what an escape would say"; exit 1; }\nexit 0\n`
            : "exec sleep 30\n"),
      );
      chmodSync(fake, 0o755);
    }
    const result = spawnSync("bash", [join(root, "tools/sandbox-scenarios.sh"), set], {
      encoding: "utf8",
      env: { ...process.env, PATH: `${dir}:${process.env.PATH}`, RUNNER_TEMP: dir, ...env },
    });
    const calls = existsSync(log) ? readFileSync(log, "utf8").trim().split("\n").filter(Boolean) : [];
    return { status: result.status, output: result.stdout + result.stderr, calls };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("the scenarios' nextest is ci.yml's", () => {
  const pinned = /NEXTEST_VERSION: "([^"]+)"/.exec(read(".github/workflows/ci.yml"))[1];
  assert.match(read(".github/actions/sandbox-scenarios/action.yml"), new RegExp(`cargo-nextest@${pinned.replaceAll(".", "\\.")}\\n`));
});

test("every entry of the nightly list is one the nightly action runs", () => {
  const known = JSON.parse(/fromJSON\('(\[[^']+\])'\)/.exec(read(".github/actions/nightly/action.yml"))[1]);
  const action = code(read(".github/actions/nightly/action.yml"));
  for (const name of known) assert.match(action, new RegExp(`inputs\\.entry == '${name}'`), name);
  const listed = read(".github/nightly-on-phase-1-proven.txt")
    .split("\n")
    .map((line) => line.replace(/#.*/, "").trim().split(/\s+/)[0])
    .filter(Boolean);
  for (const entry of listed) assert.ok(known.includes(entry), `${entry} is not an entry the action knows`);
});
