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
  assert.match(macos.text, /if: needs\.plan\.outputs\.macos != 'false'/);
  // Pinned macOS versions, never `macos-latest`.
  const versions = /os: \[([^\]]+)\]/.exec(macos.text)?.[1].split(",").map((s) => s.trim());
  assert.ok(versions?.length >= 1, "a matrix of macOS versions");
  for (const os of versions) assert.match(os, /^macos-\d+$/);
});

// The checks main's merge queue requires (the workflow's header names them for the gate).
const REQUIRED = {
  "sandbox-scenarios-linux": ["sandbox-scenarios-linux"],
  "sandbox-scenarios-macos": ["sandbox-scenarios-macos (macos-15)", "sandbox-scenarios-macos (macos-26)"],
};
const LEG = { "sandbox-scenarios-linux": "linux", "sandbox-scenarios-macos": "macos" };

test("every_required_check_name_reports_on_an_unlabelled_pr_into_phase_1_proven_and_main", () => {
  const text = code(read(WORKFLOW));
  assert.match(text, /pull_request:\n\s+branches: \[phase-1-proven, main\]/);
  // An unlabelled pull request, into either branch, plans no macOS leg ...
  for (const base of ["phase-1-proven", "main"]) {
    assert.equal(pr(["crates/purlis-core/src/sandbox.rs"], [], { base }).macos, false, base);
  }
  // ... and still every required check reports: no required job is skipped by its plan, its
  // matrix is written out, so it always expands, and each step past the reason is what skips.
  for (const [id, names] of Object.entries(REQUIRED)) {
    const j = job(id);
    assert.equal(j.if, "${{ !cancelled() }}", `${id} runs whatever the plan says`);
    const os = /\n {6}matrix:\n {8}os: \[([^\]]+)\]/.exec(j.text)?.[1].split(",").map((v) => v.trim());
    const reported = os ? os.map((v) => j.name.replace("${{ matrix.os }}", v)) : [j.name];
    assert.deepEqual(reported, names, id);
    const [why, ...rest] = j.text.split(/\n(?= {6}- )/).slice(1);
    assert.match(why, /needs\.plan\.outputs\.why/, id);
    assert.doesNotMatch(why, /\bif:/, `${id} always says why`);
    for (const step of rest) {
      assert.match(step, new RegExp(`if: ((failure|always)\\(\\) && )?needs\\.plan\\.outputs\\.${LEG[id]} != 'false'\\n`), step);
    }
  }
  // The names the gate is told to require are the ones the jobs report.
  for (const name of Object.values(REQUIRED).flat()) {
    const generic = name.replace(/\(macos-\d+\)/, "(<os>)");
    assert.ok(read(WORKFLOW).includes(`\`${generic}\``), `the header names ${generic}`);
  }
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

test("a red scenario run fails and prints what it said: nothing runs quiet (Q301)", () => {
  const red = runScript("suite", { FAKE_CARGO_FAILS: "1", QUIET: "true" });
  assert.notEqual(red.status, 0);
  assert.match(red.output, /what an escape would say/);
  // Red under load stays red, and every one of its twenty runs happens, so their verdicts compare.
  const stressed = runScript("suite", { FAKE_CARGO_FAILS: "1", STRESS: "true" });
  assert.notEqual(stressed.status, 0);
  assert.equal(stressed.calls.filter((c) => c.startsWith("cargo ")).length, 20);
  for (const path of [
    "tools/sandbox-scenarios.sh",
    ".github/actions/sandbox-scenarios/action.yml",
    ".github/actions/nightly/action.yml",
    WORKFLOW,
  ]) {
    // The quiet run's switch, its input and its matrix field; `stress-ng --quiet` is no quiet run.
    assert.doesNotMatch(read(path), /QUIET|quiet:|\.quiet\b|Q287|Q298/, path);
    assert.doesNotMatch(read(path), /mirror/i, path);
  }
});

test("the scenarios are purlis-app's one-word scenarios module, and the script's filters name it", () => {
  assert.match(read("app/src-tauri/src/lib.rs"), /#\[cfg\(test\)\]\nmod scenarios;\n/);
  assert.ok(existsSync(join(root, "app/src-tauri/src/scenarios.rs")));
  assert.match(runScript("suite").calls[0], /test\(\/\^scenarios::\/\) & not test\(\/\^scenarios::production_timings::\/\)/);
  assert.match(runScript("production-timings").calls[0], /test\(\/\^scenarios::production_timings::\/\)/);
  // The resolver seam's own tests bind local ports, so the suite runs them on each OS's leg (T1.1-02).
  assert.match(runScript("suite").calls[0], /\| binary_id\(purlis-scenarios::resolver_seam\)/);
  assert.match(runScript("suite").calls[0], /\| binary_id\(purlis-scenarios::fake_forge\)/);
  assert.doesNotMatch(runScript("production-timings").calls[0], /resolver_seam/);
  assert.doesNotMatch(runScript("production-timings").calls[0], /fake_forge/);
});

test("the Linux scenario build is the rust job's test build, so it restores the rust job's cache", () => {
  const rust = jobsOf(read(".github/workflows/ci.yml")).find((j) => j.id === "rust");
  const built = /run: cargo nextest run (.*) --no-run\n/.exec(rust.text)[1];
  const action = code(read(".github/actions/sandbox-scenarios/action.yml"));
  // The rust job keys its cache by its id, `rust`; the action restores that key on Linux.
  assert.doesNotMatch(rust.text, /shared-key/);
  assert.match(action, /shared-key: \$\{\{ runner\.os == 'macOS' && 'app' \|\| 'rust' \}\}/);
  // The same packages build and run, so the run reuses the build.
  assert.match(action, /CARGO_PACKAGES: \$\{\{ runner\.os == 'Linux' && '--workspace' \|\| '-p purlis-app -p purlis-scenarios -p fake-harness -p purlis-cli' \}\}/);
  assert.equal(built, "--workspace --locked");
  assert.match(action, /cargo nextest run \$CARGO_PACKAGES --locked --no-run/);
  const linux = runScript("suite", { CARGO_PACKAGES: "--workspace" });
  assert.match(linux.calls[0], /^cargo nextest run --workspace --locked /);
  assert.match(runScript("suite").calls[0], /^cargo nextest run -p purlis-app -p purlis-scenarios --locked /);
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
            ? `case " $* " in *" list "*) printf '%s\\n' "$FAKE_LIST"; exit 0;; esac\n` +
              // What nextest does: each call rewrites one JUnit file, with the tests it ran.
              `if [ -n "$FAKE_JUNIT" ]; then b=$(echo "$*" | sed -n 's/.*test(\\/^\\([a-z_:]*\\)(.*/\\1/p'); mkdir -p target/nextest/ci; ` +
              `printf '<testsuite>\\n<testcase classname="purlis-app" name="%s::m_01"/>\\n<testcase classname="purlis-app" name="%s::m_02"/>\\n</testsuite>\\n' "$b" "$b" > target/nextest/ci/junit.xml; fi\n[ -n "$FAKE_CARGO_FAILS" ] && { echo "what an escape would say"; exit 1; }\nexit 0\n`
            : "exec sleep 30\n"),
      );
      chmodSync(fake, 0o755);
    }
    const result = spawnSync("bash", [join(root, "tools/sandbox-scenarios.sh"), set], {
      encoding: "utf8",
      // A stand-in that writes JUnit files runs in the folder of its own, not the checkout.
      cwd: env.FAKE_JUNIT ? dir : undefined,
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

test("the nightly runs the set three times, its batches shuffled each time and its cells never", () => {
  const list = ["a", "b", "c", "d"].flatMap((b) => ["m_01", "m_02"].map((c) => `purlis-app scenarios::${b}::${c}`)).join("\n");
  const nightly = runScript("suite", { REPEAT: "3", SHUFFLE_SEED: "11", FAKE_LIST: list });
  assert.equal(nightly.status, 0, nightly.output);
  const runs = nightly.calls.filter((c) => c.startsWith("cargo nextest run"));
  // Four batches in each of three runs; each names one batch, whole, with no cell in it.
  assert.equal(runs.length, 12, runs.join("\n"));
  const batches = runs.map((c) => /test\(\/\^scenarios::(\w)\(::\|\$\)\/\)/.exec(c)?.[1]);
  assert.ok(batches.every(Boolean), runs.join("\n"));
  const orders = [0, 1, 2].map((i) => batches.slice(i * 4, i * 4 + 4).join(""));
  for (const order of orders) assert.deepEqual([...order].sort(), ["a", "b", "c", "d"]);
  assert.ok(new Set(orders).size > 1, `the order moved between runs: ${orders}`);
  for (const c of runs) assert.match(c, /--retries 0\b/);
});

test("an all-green nightly, its batches shuffled, has no entry that differs between its runs", () => {
  // nextest rewrites one JUnit file a call: the run's verdicts are every batch's, not the last one's.
  const list = ["a", "b", "c", "d"].flatMap((b) => ["m_01", "m_02"].map((c) => `purlis-app scenarios::${b}::${c}`)).join("\n");
  const nightly = runScript("suite", { REPEAT: "3", SHUFFLE_SEED: "11", FAKE_LIST: list, FAKE_JUNIT: "1", FLAKY_DRY_RUN: "1" });
  assert.equal(nightly.status, 0, nightly.output);
  assert.match(nightly.output, /3 runs compared: 0 entries differ/);
});

test("the stress run keeps M-NET-25 out", () => {
  const stress = runScript("suite", { STRESS: "true" });
  assert.match(stress.calls.find((c) => c.startsWith("cargo")), /not test\(\/m_net_25\/\)/);
  assert.equal(stress.calls.filter((c) => c.startsWith("cargo nextest run")).length, 20);
  // Not the production-timing set's nightly run either: that one is the nightly's alone.
  assert.doesNotMatch(runScript("suite").calls[0], /m_net_25/);
});

test("the_merge_queue_runs_the_production_timing_entries: both OSes, as the queue's own trigger", () => {
  const queue = plan({ event: "merge_group" });
  assert.equal(queue.productionTimings, true);
  assert.equal(queue.linux && queue.macos, true);
  assert.equal(queue.trigger, "merge_group", "M-NET-25 is the one cell it skips, and only for that trigger (Q300)");
  // A pull request's label run is its own trigger; an unlabelled one runs none.
  assert.equal(pr(["crates/purlis-core/src/lib.rs"], ["production-timings"]).trigger, "label");
  assert.equal(pr(["crates/purlis-core/src/lib.rs"]).trigger, "pull_request");
  assert.equal(pr(["crates/purlis-core/src/lib.rs"]).productionTimings, false);
  // Every job reads the plan's trigger for its gate step, and the scenarios action passes it on.
  const linux = job("sandbox-scenarios-linux");
  const macos = job("sandbox-scenarios-macos");
  for (const j of [linux, macos]) assert.match(j.text, /trigger: \$\{\{ needs\.plan\.outputs\.trigger \}\}/);
  assert.match(code(read(".github/actions/sandbox-scenarios/action.yml")), /SCENARIOS_TRIGGER: \$\{\{ inputs\.trigger \}\}/);
});

test("each budget is a job's timeout, and the doc names the runs that measured it", () => {
  const doc = read("docs/sandbox-scenarios-ci.md");
  for (const id of ["sandbox-scenarios-linux", "sandbox-scenarios-macos", "macos-sandbox-tests", "linux-release-profile"]) {
    const minutes = /timeout-minutes: (?:\$\{\{.*\|\| )?(\d+)/.exec(job(id).text)?.[1];
    assert.ok(minutes, `${id} has a budget`);
    assert.match(doc, new RegExp("\\| `" + id + "` \\| " + minutes + " \\|"), `${id}: the doc's table says ${minutes}`);
  }
  assert.match(doc, /actions\/runs\/\d+/, "the doc cites runs");
});
