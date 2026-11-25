// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `release.yml`'s dev builds: started by the push to `main` itself, so the run's own commit is
// the one it builds and the one its provenance names; built only once that commit's `ci` ended
// green; queued, never cancelled, and never published over a newer build. Run with
// `node --test tools/`.
//
// Until November 2026 a dev build was started by `ci`'s end (`workflow_run`). Such a run's
// `github.sha` is main's head when it starts, not the commit it builds, and the provenance took
// the former. And on 2026-10-13 run 654 published a commit nine commits older than the one the
// channel served. These hold the trigger, the group and the gates to the shape that stops both,
// by working out what the file's own expressions answer for each way in where they can, so a
// later edit that words them differently still passes and one that changes what they answer
// does not.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { lstatSync, mkdirSync, mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { publishedBuild } from "./dev-build-order.mjs";

const REPO = "purlis/purlis";
const text = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
const lines = text.split("\n");

/** The lines under the top-level `key:`, comments and blank lines dropped. */
function block(key) {
  const start = lines.indexOf(`${key}:`);
  assert.notEqual(start, -1, `release.yml has no top-level \`${key}:\``);
  const body = [];
  for (const line of lines.slice(start + 1)) {
    if (line.trim() === "" || line.trim().startsWith("#")) continue;
    if (!line.startsWith(" ")) break;
    body.push(line);
  }
  return body;
}

/** The lines of the job `name`, comments dropped. */
function job(name) {
  const jobs = block("jobs");
  const start = jobs.indexOf(`  ${name}:`);
  assert.notEqual(start, -1, `release.yml has no \`${name}\` job`);
  const end = jobs.findIndex((line, i) => i > start && /^ {2}\S/.test(line));
  return jobs.slice(start + 1, end === -1 ? undefined : end);
}

/** The value written after `key:` on one line of `body`, at `indent` spaces. */
function value(body, indent, key) {
  const prefix = `${" ".repeat(indent)}${key}:`;
  const found = body.filter((line) => line.startsWith(prefix));
  assert.equal(found.length, 1, `expected one \`${key}:\` at ${indent} spaces: ${found}`);
  return found[0].slice(prefix.length).trim();
}

/**
 * What a workflow expression answers for `github`.
 *
 * Only what these expressions use: `==`, `!=`, `&&`, `||`, brackets, single-quoted
 * strings, `format` and paths into `github` and `needs`. `&&` and `||` answer one of their operands, as
 * JavaScript's do, and a path that leads nowhere is null. Anything else in the expression
 * fails the test: it has to be taught here before the file may use it.
 */
function evaluate(expression, github, needs = {}) {
  const parts = expression.split(/('(?:[^']|'')*')/);
  const script = parts
    .map((part, i) => {
      if (i % 2 === 1) return JSON.stringify(part.slice(1, -1).replaceAll("''", "'"));
      assert.match(part, /^[\w\s.()!=&|,]*$/, `an expression these tests cannot read: ${part}`);
      return part
        .replace(/==/g, "===")
        .replace(/!=(?!=)/g, "!==")
        .replace(/\b(github|needs)((?:\.\w+)+)/g, (_, root, path) => `(${root}${path.replaceAll(".", "?.")} ?? null)`);
    })
    .join("");
  const format = (template, ...args) => template.replace(/\{(\d+)\}/g, (_, n) => String(args[n]));
  return new Function("github", "needs", "format", `"use strict"; return (${script});`)(github, needs, format);
}

/** A `github` context for a `release` run started by `event_name` on `ref`. */
function at(event_name, ref, runId = 3) {
  return { event_name, repository: REPO, ref, run_id: runId, sha: "8781c167".padEnd(40, "0"), event: {} };
}

/** A push to this repository's `main`: the one way in that builds a dev build. */
const MAIN = () => at("push", "refs/heads/main");

/** Every way a `release` run starts, and whether it is a dev build, which waits for `ci`. */
const WAYS_IN = [
  { name: "a push to main", github: MAIN(), dev: true },
  { name: "a pushed tag", github: at("push", "refs/tags/v1.2.3"), dev: false },
  { name: "the nightly profile guard", github: at("schedule", "refs/heads/main"), dev: false },
  { name: "a build started by hand on main", github: at("workflow_dispatch", "refs/heads/main"), dev: false },
  { name: "a build started by hand on a branch", github: at("workflow_dispatch", "refs/heads/try"), dev: false },
];

const concurrency = block("concurrency");
const group = () => {
  const written = value(concurrency, 2, "group");
  const found = /^([\w-]*)\$\{\{ (.*) \}\}$/.exec(written);
  assert.ok(found, `the group is one prefix and one expression: ${written}`);
  return (github) => `${found[1]}${evaluate(found[2], github)}`;
};

/** The expression of a `${{ }}` written after `key:` at `indent` spaces of `body`. */
function expression(body, indent, key) {
  const written = value(body, indent, key);
  const found = /^\$\{\{ (.*) \}\}$/.exec(written);
  assert.ok(found, `${key} is one expression: ${written}`);
  return found[1];
}

// Under `workflow_run`, `github.sha` is main's head when the run starts, and that is the
// commit the OIDC token, and so the provenance, names, though the run built the commit its `ci`
// ran on. A run started by the push itself has the two the same.
test("a release run is started by a push to main or a tag, the nightly guard and by hand, never by another run's end", () => {
  const on = block("on");
  const keys = on.filter((line) => /^ {2}\S/.test(line)).map((line) => line.trim());
  assert.deepEqual(keys, ["workflow_dispatch:", "push:", "schedule:"]);
  const push = on.slice(on.indexOf("  push:") + 1, on.indexOf("  schedule:"));
  assert.deepEqual(push, ['    tags: ["v*"]', "    branches: [main]"]);
  assert.ok(!text.includes("workflow_run"), "release.yml still names workflow_run");
});

test("a build in progress is never cancelled", () => {
  assert.equal(value(concurrency, 2, "cancel-in-progress"), "false");
  assert.deepEqual(
    concurrency.map((line) => line.trim().split(":")[0]),
    ["group", "cancel-in-progress"],
  );
  // And no job takes a group of its own that could cancel one.
  assert.deepEqual(
    block("jobs").filter((line) => /^\s+(concurrency|cancel-in-progress):/.test(line)),
    [],
  );
});

test("every push to main waits in the dev builds' group, and every other way in has a group of its own", () => {
  const groupOf = group();
  assert.equal(groupOf(MAIN()), "release-main");
  assert.equal(groupOf(at("push", "refs/heads/main", 9)), "release-main");
  assert.equal(groupOf(at("schedule", "refs/heads/main")), "release-profile-guard");
  assert.equal(groupOf(at("push", "refs/tags/v1.2.3")), "release-refs/tags/v1.2.3");
  assert.equal(groupOf(at("workflow_dispatch", "refs/heads/main")), "release-refs/heads/main");
  assert.equal(groupOf(at("workflow_dispatch", "refs/heads/try")), "release-refs/heads/try");
});

test("the gate is the first job, holds nothing but the right to read runs, and passes every way in but a push to main at once", () => {
  const names = block("jobs").filter((line) => /^ {2}\S/.test(line)).map((line) => line.trim());
  assert.equal(names[0], "gate:");
  const gate = job("gate");
  assert.ok(!gate.some((line) => line.startsWith("    needs:")), "the gate waits on a job");
  assert.ok(!gate.some((line) => line.startsWith("    environment:")), "the gate takes an environment");
  assert.ok(!gate.some((line) => line.trim().startsWith("- uses: actions/checkout")), "the gate checks out code");
  const permissions = gate.slice(gate.indexOf("    permissions:") + 1);
  assert.equal(permissions[0], "      actions: read");
  assert.ok(!/^ {6}\S/.test(permissions[1] ?? ""), `the gate holds more: ${permissions[1]}`);
  for (const { name, github, dev } of WAYS_IN) {
    assert.equal(evaluate(expression(gate, 10, "DEV"), github), dev, name);
  }
});

test("the gate answers green only when this commit's own ci, on its push to main, ended success", () => {
  const gate = job("gate").map((line) => line.trim());
  const order = [
    'if [ "$DEV" != true ]; then',
    'echo "green=true" >> "$GITHUB_OUTPUT"',
    "exit 0",
    // One call reads both, three tries; a cancelled ci is `green=false`, never green.
    'if out=$(gh run view "$1" --repo "$GITHUB_REPOSITORY" --json status,conclusion --jq \'.status + " " + (.conclusion // "none")\'); then',
    'id=$(gh run list --repo "$GITHUB_REPOSITORY" --workflow ci.yml --commit "$GITHUB_SHA" --event push --branch main --limit 1 --json databaseId --jq \'.[0].databaseId // empty\') || id=""',
    'gh run watch "$id" --repo "$GITHUB_REPOSITORY" --interval 30 --exit-status > /dev/null || true',
    'state=$(ask "$id") || { echo "::error::could not read ci run $id of $GITHUB_SHA, so nothing is built"; exit 1; }',
    'read -r status conclusion <<< "$state"',
    '[ "$status" = completed ] || { echo "::error::ci run $id of $GITHUB_SHA is $status after the wait, so nothing is built"; exit 1; }',
    'if [ "$conclusion" = success ]; then',
    'echo "green=true" >> "$GITHUB_OUTPUT"',
    "else",
    'echo "green=false" >> "$GITHUB_OUTPUT"',
  ];
  let from = 0;
  for (const line of order) {
    const found = gate.indexOf(line, from);
    assert.notEqual(found, -1, `the gate lacks, or has out of order: ${line}`);
    from = found + 1;
  }
  assert.ok(gate.includes("GH_TOKEN: ${{ github.token }}"));
  // A run that never appears is an error, never a pass.
  assert.ok(gate.some((line) => line.startsWith('[ -n "$id" ] || { echo "::error::')));
});

test("plan does not wait for the gate, and what signs, uploads or publishes runs only when the gate answered green", () => {
  // R2 (#7381): `plan` and `keys` hold nothing, and `build` compiles while `ci` runs. The jobs
  // that publish are behind the gate; `crates/release-manifest` holds each of them on a workflow
  // broken on purpose.
  const plan = job("plan");
  for (const key of ["needs", "if", "environment"]) {
    assert.ok(!plan.some((line) => line.startsWith(`    ${key}:`)), `plan has \`${key}:\``);
  }
  assert.equal(value(job("keys"), 4, "needs"), "plan");
  assert.equal(value(job("build"), 4, "needs"), "[plan, keys]");
  const condition = value(job("runner-image"), 4, "if");
  assert.equal(value(job("runner-image"), 4, "needs"), "[gate, plan, keys]");
  for (const [green, publish, runs] of [
    ["true", "true", true],
    ["false", "true", false],
    ["", "true", false],
    ["true", "false", false],
  ]) {
    const needs = { gate: { outputs: { green } }, keys: { outputs: { publish } } };
    assert.equal(evaluate(condition, {}, needs), runs, `green=${green} publish=${publish}`);
  }
});

test("a dev build's commit is checked against main's history before anything from the checkout runs", () => {
  const plan = job("plan").map((line) => line.trim());
  const script = plan.indexOf("set -euo pipefail");
  assert.deepEqual(plan.slice(script + 1, script + 8), [
    'if [ "$DEV" = true ]; then',
    'on_main=$(gh api "repos/$GITHUB_REPOSITORY/compare/$SHA...main" --jq .status)',
    'case "$on_main" in',
    'ahead | identical) echo "$SHA is in main\'s history" ;;',
    "*) echo \"::error::$SHA is not in main's history (main is '$on_main' of it), so it is not built\"; exit 1 ;;",
    "esac",
    "fi",
  ]);
  // Nothing of the repository's own runs before it in `plan`: the checkout, then this.
  const before = plan.slice(0, script).filter((line) => /^- (uses|name|run):|^run:|^uses:/.test(line));
  assert.deepEqual(before.map((line) => line.split("@")[0]), [
    "- uses: actions/checkout",
    "- name: Channel, version, and whether anything is published",
    "run: |",
  ]);
});

test("a dev build is the commit its green ci ran on, the run's own commit, and no other", () => {
  const plan = job("plan");
  const sha = "${{ github.sha }}";
  // The gate waited on the ci run of `$GITHUB_SHA`: the same commit.
  assert.ok(job("gate").some((line) => line.includes('--commit "$GITHUB_SHA"')));
  assert.equal(expression(plan, 10, "DEV"), "github.event_name == 'push' && github.ref == 'refs/heads/main'");
  // The first checkout of `plan`, and the `SHA` every later job checks out as `plan`'s `sha`.
  assert.equal(plan.find((line) => line.trim().startsWith("ref:"))?.trim(), `ref: ${sha}`);
  assert.ok(plan.includes(`          SHA: ${sha}`), "plan's SHA is not the ci run's commit");
  assert.ok(plan.includes('            echo "sha=$SHA"; echo "tag=$tag"; echo "skip=$skip"'));
  for (const name of ["build", "sbom", "manifest", "publish"]) {
    const refs = job(name).filter((line) => line.trim().startsWith("ref:"));
    assert.deepEqual(
      refs.map((line) => line.trim()),
      ["ref: ${{ needs.plan.outputs.sha }}"],
      name,
    );
  }
});

test("plan asks whether the dev channel already serves this commit or a later one", () => {
  const plan = job("plan").map((line) => line.trim());
  const ask = plan.indexOf(
    'order=$(node tools/dev-build-order.mjs --version "$version" --sha "$SHA")',
  );
  assert.notEqual(ask, -1, "plan does not ask tools/dev-build-order.mjs");
  assert.equal(plan[ask - 1], 'if [ "$DEV" = true ] && [ "$skip" != "true" ]; then');
  assert.deepEqual(plan.slice(ask + 1, ask + 8), [
    'case "$order" in',
    'publish:*) echo "$order" ;;',
    "skip:*)",
    'echo "::notice::not built, and nothing is published: ${order#skip: }"',
    "skip=true; publish=false ;;",
    '*) echo "::error::tools/dev-build-order.mjs answered neither publish nor skip: $order"; exit 1 ;;',
    "esac",
  ]);
  // Asked before the outputs are written, in a script that stops at the first failure.
  assert.ok(ask < plan.indexOf('} >> "$GITHUB_OUTPUT"'));
  assert.ok(plan.includes("GH_TOKEN: ${{ github.token }}"));
  // And `skip` is what keeps every build, and so every publish, from starting.
  assert.equal(value(job("keys"), 4, "if"), "needs.plan.outputs.skip != 'true'");
  assert.equal(value(job("build"), 4, "if"), "needs.keys.outputs.skip != 'true'");
  assert.equal(
    value(job("publish"), 4, "needs"),
    "[gate, plan, keys, build, runner-binaries, manifest, provenance]",
  );
});

test("the publish job asks again before it replaces the first dev asset, and stops unless told to publish", () => {
  // "Re-run failed jobs" on an old run keeps `plan`'s answer and runs only `publish`.
  const publish = job("publish").map((line) => line.trim());
  const step = publish.indexOf("- name: Dev — replace the assets of the one `dev` prerelease");
  assert.notEqual(step, -1, "publish has no dev step");
  const ask = publish.indexOf(
    'order=$(node tools/dev-build-order.mjs --version "$VERSION" --sha "$SHA")',
  );
  assert.ok(ask > step, "the dev step does not ask tools/dev-build-order.mjs");
  assert.deepEqual(publish.slice(ask + 1, ask + 5), [
    'case "$order" in',
    'publish:*) echo "$order" ;;',
    '*) echo "::error::not published, and no asset was replaced: ${order#skip: }"; exit 1 ;;',
    "esac",
  ]);
  assert.ok(publish.slice(step, ask).includes("set -euo pipefail"));
  // Nothing of the dev release is written, replaced or deleted before the answer.
  const writes = publish
    .map((line, i) => [line, i])
    .filter(([line, i]) => i > step && /replace-release-assets\.sh dev|gh release edit dev|-X DELETE/.test(line));
  assert.ok(writes.length >= 4, `the dev step's writes were not found: ${writes}`);
  for (const [line, i] of writes) assert.ok(i > ask, `written before the order is asked: ${line}`);
  // The job has what the tool needs: the token, and the version and commit `plan` gave.
  for (const want of [
    "GH_TOKEN: ${{ github.token }}",
    "VERSION: ${{ needs.plan.outputs.version }}",
    "SHA: ${{ needs.plan.outputs.sha }}",
  ]) {
    assert.ok(publish.includes(want), `publish lacks ${want}`);
  }
});

test("every job has a deadline, so a hung one cannot hold the dev builds' group for six hours", () => {
  const names = block("jobs")
    .filter((line) => /^ {2}\S/.test(line))
    .map((line) => line.trim().slice(0, -1));
  assert.deepEqual(names, [
    "gate",
    "plan",
    "keys",
    "build",
    "runner-image",
    "runner-image-signature",
    "runner-binary-aarch64",
    "runner-binaries",
    "sbom",
    "manifest",
    "provenance",
    "publish",
    "symbols",
    "profile-guard",
  ]);
  // A job that calls a workflow has no deadline of its own: it takes the longest chain of
  // the called workflow's jobs, each of which has one (#2749).
  const called = (body) => {
    const uses = body.find((line) => line.startsWith("    uses: ./.github/workflows/"));
    if (!uses) return null;
    const other = readFileSync(new URL(`../${uses.trim().slice("uses: ./".length)}`, import.meta.url), "utf8");
    return deadlines(other.split("\n"));
  };
  const minutesOf = new Map();
  for (const name of names) {
    const body = job(name);
    const inner = called(body);
    if (inner !== null) {
      assert.ok(!body.some((line) => line.startsWith("    timeout-minutes:")), name);
      minutesOf.set(name, inner);
      continue;
    }
    const minutes = Number(value(body, 4, "timeout-minutes"));
    // The build is one long job: a cold macOS build took 65 to 68 minutes (ADR 0092), and 106 to
    // 107 by 2026-11-22 (run 45030876196 hit its old 120 and was cancelled). The gate
    // is the other: it waits for this commit's `ci`, which took up to 111 minutes on a push to
    // main in the first week of November 2026.
    // `runner-binary-aarch64` is the third: it compiles, then waits for the same `ci` (R2).
    const most = { build: 150, gate: 150, "runner-binary-aarch64": 150 }[name] ?? 90;
    assert.ok(Number.isInteger(minutes) && minutes >= 10 && minutes <= most, `${name}: ${minutes}`);
    minutesOf.set(name, minutes);
  }
  // The longest a run can hold the group: the slowest chain of jobs, each waiting on those it
  // needs, every one at its deadline.
  const longest = chain(names, minutesOf, (name) => needsOf(job(name)));
  assert.ok(longest <= 400, `a run may hold the group for ${longest} minutes`);
});

/** The jobs `body` waits on. */
function needsOf(body) {
  const line = body.find((l) => l.startsWith("    needs:"));
  if (!line) return [];
  const value = line.slice("    needs:".length).trim();
  return value.startsWith("[") ? value.slice(1, -1).split(",").map((n) => n.trim()) : [value];
}

/** The longest chain through `names`, by `minutesOf`, each after what `needs` names. */
function chain(names, minutesOf, needs) {
  const done = new Map();
  const finish = (name) => {
    if (!done.has(name)) {
      const before = Math.max(0, ...needs(name).map(finish));
      done.set(name, before + minutesOf.get(name));
    }
    return done.get(name);
  };
  return Math.max(...names.map(finish));
}

/** The longest chain of deadlines through a workflow's jobs, given its lines. */
function deadlines(other) {
  const start = other.indexOf("jobs:");
  const bodies = new Map();
  let current = null;
  for (const line of other.slice(start + 1)) {
    if (/^\S/.test(line)) break;
    if (/^ {2}[\w-]+:$/.test(line)) {
      current = line.trim().slice(0, -1);
      bodies.set(current, []);
    } else if (current && !line.trim().startsWith("#")) {
      bodies.get(current).push(line);
    }
  }
  const names = [...bodies.keys()];
  const minutesOf = new Map(names.map((n) => [n, Number(value(bodies.get(n), 4, "timeout-minutes"))]));
  return chain(names, minutesOf, (name) => needsOf(bodies.get(name)));
}

test("the tool reads the commit from the notes the manifest job writes, and the version plan gives", () => {
  const manifest = job("manifest").map((line) => line.trim());
  const notes = 'notes="purlis $VERSION, a dev build of $SHA."';
  assert.ok(manifest.includes(notes), `the manifest job no longer writes ${notes}`);
  const sha = "0854cd21".padEnd(40, "f");
  const written = notes
    .slice('notes="'.length, -1)
    .replace("$VERSION", "0.4.2-dev.663")
    .replace("$SHA", sha);
  assert.deepEqual(publishedBuild(JSON.stringify({ version: "0.4.2-dev.663", notes: written })), {
    version: "0.4.2-dev.663",
    sha,
  });
  // The number only grows, and runs in one group publish in the order they were numbered.
  const plan = job("plan").map((line) => line.trim());
  assert.ok(plan.includes('tag=""; version="$base-dev.$GITHUB_RUN_NUMBER"'));
});

test("the runs list names the commit a dev build builds", () => {
  const name = /^\$\{\{ (.*) \}\}$/.exec(value(lines, 0, "run-name"));
  assert.ok(name, "run-name is one expression");
  const github = { ...MAIN(), ref_name: "main" };
  assert.equal(evaluate(name[1], github), `dev build of ${github.sha}`);
  assert.equal(
    evaluate(name[1], { event_name: "push", ref: "refs/tags/v1.2.3", ref_name: "v1.2.3", event: {} }),
    "stable v1.2.3",
  );
  assert.equal(evaluate(name[1], { event_name: "schedule", event: {} }), "profile guard");
  assert.equal(
    evaluate(name[1], { event_name: "workflow_dispatch", ref_name: "try", event: {} }),
    "on demand, try",
  );
});

// Every dev build on main from 2026-11-05T21:36Z lost its Linux runner while compiling
// `purlis-app`: the build needs about 26 GB and the runner has 16 of memory and 3 of swap (#5277).
test("the Linux build turns on more swap before anything compiles", () => {
  const steps = job("build").join("\n");
  const swap = steps.indexOf("swapon /swapfile-build");
  assert.notEqual(swap, -1, "the build job turns on no swap file");
  const head = steps.slice(0, swap);
  const name = head.lastIndexOf("- name:");
  assert.match(head.slice(name), /if: runner\.os == 'Linux'/);
  assert.ok(swap < steps.indexOf("rust-toolchain"), "the swap is turned on before the toolchain is installed");
  assert.ok(swap < steps.indexOf("Compile the app, the purlis binary"), "the swap is turned on before anything compiles");
});

test("the debug symbols go up in a job of their own that never holds a release back (#5692)", () => {
  const symbols = job("symbols");
  // After the build, from what it saved, so the build's own time is untouched.
  assert.deepEqual(needsOf(symbols), ["gate", "plan", "keys", "build"]);
  // Whenever the run publishes, even when one platform's build failed and the other's saved
  // its symbols; never in a cancelled run.
  assert.equal(
    value(symbols, 4, "if"),
    "${{ !cancelled() && needs.gate.outputs.green == 'true' && needs.keys.outputs.publish == 'true' && needs.keys.result == 'success' }}",
  );
  // A red here leaves the run green, and nothing that publishes waits on it.
  assert.equal(value(symbols, 4, "continue-on-error"), "true");
  for (const name of ["sbom", "manifest", "provenance", "publish", "profile-guard"]) {
    assert.ok(!needsOf(job(name)).includes("symbols"), `${name} waits on the symbols`);
  }
  const body = symbols.map((line) => line.trim());
  // The intake's secrets, under the names sentry-cli reads; nothing else of the release's.
  for (const want of [
    "SENTRY_AUTH_TOKEN: ${{ secrets.SENTRY_SYMBOLS_TOKEN }}",
    "SENTRY_ORG: ${{ secrets.SENTRY_ORG }}",
    "SENTRY_PROJECT: ${{ secrets.SENTRY_PROJECT }}",
  ]) {
    assert.ok(body.includes(want), `symbols lacks ${want}`);
  }
  const secrets = body.join("\n").match(/secrets\.\w+/g);
  assert.deepEqual([...new Set(secrets)].sort(), [
    "secrets.SENTRY_ORG",
    "secrets.SENTRY_PROJECT",
    "secrets.SENTRY_SYMBOLS_TOKEN",
  ]);
  // Every platform's symbols artifact, by pattern: a build that saved none leaves none to
  // download, and the tool says so rather than the download failing.
  assert.ok(body.includes("pattern: purlis-*-symbols-${{ needs.plan.outputs.version }}"));
  assert.ok(body.includes("run: node tools/symbols-upload.mjs check symbols"));
  assert.ok(body.includes("run: node tools/symbols-upload.mjs upload symbols"));
});

test("the macOS build saves its debug symbols as an artifact of their own, never published (#5692)", () => {
  const build = job("build").map((line) => line.trim());
  assert.ok(
    build.includes("name: purlis-${{ steps.names.outputs.slug }}-symbols-${{ needs.plan.outputs.version }}"),
    "the build uploads no symbols artifact",
  );
  // The jobs that publish download each platform's artifact by its exact name, never by a
  // pattern that would also take the symbols.
  for (const name of ["sbom", "manifest", "provenance", "publish"]) {
    assert.ok(!job(name).some((line) => line.trim().startsWith("pattern:")), name);
  }
});

test("the macOS build collects each dSYM cargo left behind its symlink, whole (#5692)", () => {
  // Cargo leaves `target/<profile>/<bin>.dSYM` as a relative symlink into `deps/`: a copy of the
  // link alone dangles, upload-artifact leaves it out, and the intake gets no symbols.
  const lines = step(job("build"), "Collect the debug symbols (macOS)");
  const script = lines.slice(lines.indexOf("run: |") + 1).join("\n");
  const dir = mkdtempSync(join(tmpdir(), "symbols-"));
  const dwarf = join(dir, "target/release/deps/purlis-0123.dSYM/Contents/Resources/DWARF");
  mkdirSync(dwarf, { recursive: true });
  writeFileSync(join(dwarf, "purlis-0123"), "dwarf");
  symlinkSync("deps/purlis-0123.dSYM", join(dir, "target/release/purlis.dSYM"));

  execFileSync("bash", ["-c", script], { cwd: dir, env: { ...process.env, PROFILE: "release" } });

  const kept = join(dir, "symbols/purlis.dSYM");
  assert.ok(lstatSync(kept).isDirectory(), "the dSYM itself, not its link");
  assert.equal(
    readFileSync(join(kept, "Contents/Resources/DWARF/purlis-0123"), "utf8"),
    "dwarf",
  );
});

/** The lines of the step `name` in `body` (a job's lines), up to the next step. */
function step(body, name) {
  const start = body.findIndex((line) => line.trim() === `- name: ${name}`);
  assert.notEqual(start, -1, `no step named ${name}`);
  const indent = body[start].indexOf("-");
  const end = body.findIndex(
    (line, i) => i > start && line.indexOf("- ") === indent && line.trim().startsWith("- "),
  );
  return body.slice(start, end === -1 ? undefined : end).map((line) => line.trim());
}

test("every compile of a build takes the Report intake's address from the release environment (#5687)", () => {
  const build = job("build");
  // The secret is reached only where the build publishes: the job's environment says so, on
  // both channels, a dev build too (its reports say channel=dev, from its profile).
  assert.equal(value(build, 4, "environment"), "${{ needs.keys.outputs.publish == 'true' && 'release' || '' }}");
  // `option_env!` reads it when purlis-core compiles: the one cargo run, Tauri's (R1, #7380).
  // With the secret absent it is empty, which is no address, so nothing is sent. The bundling
  // steps compile nothing, so they are not given it.
  for (const name of ["Compile the app, the purlis binary and the built-in extensions"]) {
    assert.ok(
      step(build, name).includes("PURLIS_REPORT_INTAKE_DSN: ${{ secrets.PURLIS_REPORT_INTAKE_DSN }}"),
      `${name} builds without the intake's address`,
    );
  }
});

// The runner binaries (#723): two are `build`'s own binaries, the third a keyless job's, and the
// job that signs them builds nothing while the key is in its environment.
test("the runner binaries are build's own and one keyless job's, signed by a job that builds nothing with the key", () => {
  const build = job("build");
  assert.ok(build.some((line) => line.trim() === "runner_target: macos-arm64"));
  assert.ok(build.some((line) => line.trim() === "runner_target: linux-x86_64"));
  const handed = step(build, "It is this platform's runner binary, saying this release's version");
  assert.ok(handed.some((line) => line.includes('tools/runner-binary-check.sh "target/$PROFILE/purlis"')));

  const arm = job("runner-binary-aarch64");
  assert.equal(value(arm, 4, "runs-on"), "ubuntu-24.04-arm");
  assert.ok(!arm.some((line) => line.includes("SIGNING")), "it names no signing key");
  assert.ok(arm.some((line) => line.includes('cargo auditable build --profile "$PROFILE" --locked -p purlis-cli')));
  // cargo-auditable is the pinned release binary, checked against its digest, not compiled.
  assert.ok(arm.some((line) => line.trim() === "tool: cargo-auditable@0.7.6"));
  assert.ok(!arm.some((line) => line.includes("cargo install")));

  const sign = job("runner-binaries");
  assert.equal(value(sign, 4, "needs"), "[gate, plan, keys, build, runner-binary-aarch64]");
  const signer = step(sign, "The signer, built with no key in reach");
  assert.ok(!signer.some((line) => line.includes("secrets.")), "no secret where it compiles");
  const signing = step(sign, "Signed with the updater's key for this version and platform");
  assert.ok(signing.some((line) => line.includes("secrets.TAURI_SIGNING_PRIVATE_KEY")));
  assert.ok(
    !signing.some((line) => /\bcargo\b/.test(line)),
    "nothing is compiled, and no build script runs, in the step that holds the key",
  );
  for (const target of ["linux-x86_64", "linux-aarch64", "macos-arm64"]) {
    assert.ok(
      sign.some((line) => line.trim() === `name: purlis-runner-unsigned-${target}-\${{ needs.plan.outputs.version }}`),
      `${target} is downloaded by its exact name`,
    );
    assert.ok(
      sign.some((line) => line.trim() === `name: purlis-runner-${target}-\${{ needs.plan.outputs.version }}`),
      `${target} is handed on by its exact name`,
    );
  }
});

test("a dev publish keeps the last dev builds' runner assets, not only its own", () => {
  const publish = job("publish").join("\n");
  assert.match(publish, /node tools\/dev-runner-assets\.mjs "\$VERSION"/);
  assert.doesNotMatch(publish, /startswith\("purlis-'"\$VERSION"'-"\)/);
});

test("plan skips a dev build that ships nothing, asks before the order is asked, and says so in the summary", () => {
  const plan = job("plan").map((line) => line.trim());
  const ships = plan.findIndex((l) => l.startsWith('why=$(GITHUB_OUTPUT="$ships_out" node tools/ci-scope.mjs ships dev "$SHA")'));
  assert.notEqual(ships, -1, "plan does not ask tools/ci-scope.mjs ships");
  // Only a dev build is asked, and never after something already skipped it.
  assert.equal(plan.slice(0, ships).findLast((l) => l.startsWith("if [")), 'if [ "$DEV" = true ] && [ "$skip" != "true" ]; then');
  assert.ok(plan.slice(ships).some((l) => l.includes("skip=true; publish=false")));
  assert.ok(plan.slice(ships, ships + 14).some((l) => l.includes(">> \"$GITHUB_STEP_SUMMARY\"")));
  assert.ok(ships < plan.indexOf('order=$(node tools/dev-build-order.mjs --version "$version" --sha "$SHA")'));
  assert.ok(ships < plan.indexOf('} >> "$GITHUB_OUTPUT"'));
});

// The macOS build may run on the operator's own Macs (docs/updating.md, "Building macOS on a
// self-hosted Mac"), chosen by one repository variable and nothing in the code.
const MACOS_RUNNER = `\${{ matrix.os == 'macos-latest' && needs.plan.outputs.channel == 'dev' && needs.plan.outputs.profile == 'dev-release' && fromJSON(vars.MACOS_RELEASE_RUNNER || '"macos-latest"') || matrix.os }}`;

test("the macOS build's runner is the repository variable's, and macos-latest when it is unset", () => {
  assert.equal(value(job("build"), 4, "runs-on"), MACOS_RUNNER);
});

test("a stable build, and the profile guard that builds like one, can never take the variable's runner", () => {
  // The guard's own condition is held to what `plan` decides for each way in: a tag is channel
  // `stable` with profile `release`; the schedule is `dev` with `release`; only a dev channel
  // build has `dev-release`. Every one but the last stays on `macos-latest`.
  const plan = job("plan").join("\n");
  assert.match(plan, /channel=stable; publish=true[\s\S]*?profile=release;/);
  assert.match(plan, /schedule\)[\s\S]*?channel=dev; publish=false\s+profile=release;/);
  assert.equal(plan.match(/profile=dev-release/g)?.length >= 1, true);
  const on = value(job("build"), 4, "runs-on");
  for (const needle of ["needs.plan.outputs.channel == 'dev'", "needs.plan.outputs.profile == 'dev-release'"]) {
    assert.ok(on.includes(needle), `build's runs-on lacks ${needle}`);
  }
  assert.ok(on.indexOf("dev-release") < on.indexOf("MACOS_RELEASE_RUNNER"), "the condition comes before the variable");
  // And the provenance checks stay as they were: no verification of a release drops the flag.
  assert.equal(text.match(/--deny-self-hosted-runners/g)?.length, 2);
});

test("the variable reaches no runner of release.yml but the build's, and no other job names a self-hosted one", () => {
  const readers = [...lines.entries()].filter(([, l]) => l.includes("MACOS_RELEASE_RUNNER") && !l.trim().startsWith("#"));
  assert.equal(readers.length, 1, "MACOS_RELEASE_RUNNER is read in one place");
  for (const [name, body] of Object.entries(Object.fromEntries([...jobsOf()]))) {
    if (name === "build") continue;
    for (const line of body.filter((l) => l.startsWith("    runs-on:"))) {
      assert.doesNotMatch(line, /self-hosted|vars\.|fromJSON/, `${name} runs-on`);
    }
  }
  assert.doesNotMatch(lines.filter((l) => !l.trim().startsWith("#")).join("\n"), /runs-on:.*self-hosted/);
});

function* jobsOf() {
  let current = null;
  let body = [];
  for (const line of block("jobs")) {
    const m = /^ {2}([\w-]+):$/.exec(line);
    if (m) {
      if (current) yield [current, body];
      current = m[1];
      body = [];
    } else body.push(line);
  }
  if (current) yield [current, body];
}

test("a persistent runner's build starts clean, keeps nothing of the last one, and drops its keychain even when it fails", () => {
  const build = job("build");
  const steps = build.join("\n").split(/\n {6}- /);
  const named = (text) => steps.find((s) => s.startsWith(`name: ${text}`));
  const wipe = named("A self-hosted runner's workspace starts empty");
  const homes = named("A self-hosted runner's Cargo, rustup and temporary folder are this job's own");
  const keychain = named("A self-hosted runner keeps no keychain, certificate or job folder");
  assert.ok(wipe && homes && keychain, "a step of the self-hosted clean-up is missing");
  for (const s of [wipe, homes]) assert.match(s, /if: runner\.environment == 'self-hosted'/);
  assert.match(keychain, /if: steps\.ci\.outputs\.green == 'true' && always\(\) && runner\.environment == 'self-hosted' && runner\.os == 'macOS'/);
  assert.match(keychain, /security delete-keychain/);
  assert.match(homes, /CARGO_HOME=/);
  assert.match(homes, /RUSTUP_HOME=/);
  assert.match(homes, /TMPDIR=/);
  // The workspace is emptied before the checkout, the homes are set before the toolchain installs,
  // and the keychain goes last.
  const order = (s) => build.join("\n").indexOf(s);
  assert.ok(order(wipe) < order("uses: actions/checkout@"));
  assert.ok(order(homes) < order("uses: ./.github/actions/rust-toolchain"));
  assert.ok(order("Wait for this commit's ci on main") < order(keychain));
  // Neither the swap file nor `sudo` may reach a Mac: both are Linux's.
  assert.match(named("More swap for the build"), /if: runner\.os == 'Linux'/);
  // (The summary step prints the person's install commands, `sudo` among them, and runs none.)
  const nonLinux = steps.filter((s) => !/runner\.os == 'Linux'|RUNNER_OS" = "Linux"/.test(s) && !s.startsWith("name: What a person sees when they install it"));
  assert.ok(nonLinux.every((s) => !/\bsudo\b|\bbrew\b/.test(s.split("\n").filter((l) => !l.trim().startsWith("#")).join("\n"))), "a step without a Linux guard calls sudo or brew");
});
