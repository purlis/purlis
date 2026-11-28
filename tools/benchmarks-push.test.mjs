// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The writers to the `benchmarks` branch** (#8014): ci.yml's bench-record, stress.yml's
// stress-record and budget-run.yml's budget-record each commit a point there through
// github-action-benchmark, and push it with `tools/benchmarks-push.sh`, which fetches and
// rebases onto a point another writer pushed meanwhile. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { jobLines } from "./workflow-jobs.mjs";

const SCRIPT = fileURLToPath(new URL("./benchmarks-push.sh", import.meta.url));
const workflows = new URL("../.github/workflows/", import.meta.url);
const ENV = {
  ...process.env,
  GIT_AUTHOR_NAME: "t",
  GIT_AUTHOR_EMAIL: "t@example.invalid",
  GIT_COMMITTER_NAME: "t",
  GIT_COMMITTER_EMAIL: "t@example.invalid",
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
  WAIT: "0",
};
const git = (cwd, ...args) => execFileSync("git", args, { cwd, env: ENV, encoding: "utf8" }).trim();

/** A remote with a `benchmarks` branch, and `n` clones of it, each on its own `benchmarks`. */
function rig(n) {
  const root = mkdtempSync(join(tmpdir(), "benchmarks-push-"));
  const remote = join(root, "remote.git");
  git(root, "init", "-q", "--bare", "-b", "main", remote);
  const seed = join(root, "seed");
  git(root, "init", "-q", "-b", "benchmarks", seed);
  git(seed, "commit", "-q", "--allow-empty", "-m", "benchmarks");
  git(seed, "push", "-q", remote, "benchmarks");
  const clones = Array.from({ length: n }, (_, i) => {
    const dir = join(root, `writer-${i}`);
    git(root, "clone", "-q", "-b", "benchmarks", remote, dir);
    return dir;
  });
  return { remote, clones };
}

/** A point committed on the clone's `benchmarks`, as the action leaves it with auto-push off. */
function point(dir, file, text) {
  mkdirSync(join(dir, file, ".."), { recursive: true });
  writeFileSync(join(dir, file), text);
  git(dir, "add", file);
  git(dir, "commit", "-q", "-m", `add ${file}`);
}

const push = (dir) => spawnSync("bash", [SCRIPT], { cwd: dir, env: ENV, encoding: "utf8" });
const remoteFile = (remote, file) => git(remote, "show", `benchmarks:${file}`);

test("a point pushed after another writer's lands on top of it, and both are kept", () => {
  const { remote, clones } = rig(2);
  const [ci, budget] = clones;
  point(ci, "bench/data.js", "ci's point\n");
  point(budget, "macos-max/data.js", "budget's point\n");
  assert.equal(push(ci).status, 0);
  const late = push(budget);
  assert.equal(late.status, 0, late.stdout + late.stderr);
  assert.equal(remoteFile(remote, "bench/data.js"), "ci's point");
  assert.equal(remoteFile(remote, "macos-max/data.js"), "budget's point");
  assert.equal(git(remote, "rev-list", "--count", "benchmarks"), "3", "the root and one commit each");
});

test("two points to the same data set are one error naming it, and the first stays", () => {
  const { remote, clones } = rig(2);
  const [one, two] = clones;
  point(one, "stress/data.js", "one\n");
  point(two, "stress/data.js", "two\n");
  assert.equal(push(one).status, 0);
  const late = push(two);
  assert.notEqual(late.status, 0);
  assert.match(late.stdout, /::error title=[^:]+::.*stress\/data\.js/);
  assert.equal(remoteFile(remote, "stress/data.js"), "one");
});

// ---- The workflows ----

const all = readdirSync(workflows).filter((f) => f.endsWith(".yml"));
const benchmarkSteps = (lines) => {
  const steps = [];
  for (const line of lines) {
    if (/^ {6}- /.test(line)) steps.push([line]);
    else if (steps.length) steps.at(-1).push(line);
  }
  return steps;
};

test("every point for the benchmarks branch is committed by the action and pushed by the script", () => {
  const writers = [];
  for (const file of all) {
    for (const [job, lines] of jobLines(readFileSync(new URL(file, workflows), "utf8"))) {
      const steps = benchmarkSteps(lines);
      const records = steps.filter((s) => s.some((l) => l.includes("uses: benchmark-action/github-action-benchmark@")));
      if (records.length === 0) continue;
      writers.push(`${file} ${job}`);
      for (const step of records) {
        assert.ok(step.some((l) => l.trim() === "gh-pages-branch: benchmarks"), `${file} ${job} writes another branch`);
        assert.ok(step.some((l) => l.trim() === "auto-push: false"), `${file} ${job}: the action pushes on its own`);
      }
      const last = steps.indexOf(records.at(-1));
      const pusher = steps.findIndex((s) => s.some((l) => l.includes("tools/benchmarks-push.sh")));
      assert.ok(pusher > last, `${file} ${job}: no push after its last record`);
    }
  }
  assert.deepEqual(writers.sort(), ["budget-run.yml budget-record", "ci.yml bench-record", "stress.yml stress-record"]);
});
