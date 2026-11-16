// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The nightly list (`.github/nightly-on-<branch>.txt`, T1.1-09): main's scheduler checks out the
// branch and turns the list into its matrix with `nightly-list.mjs`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { matrixOf, parseList } from "./nightly-list.mjs";

const LIST = new URL("../.github/nightly-on-phase-1-proven.txt", import.meta.url);
const sample = `# entry   runs-on   timeout   [kind: escape]
placeholder        ubuntu-24.04   30

refusals-hold      macos-15       90   kind: escape
`;

test("the nightly list on the branch is well formed and lists the placeholder entries", () => {
  const entries = parseList(readFileSync(LIST, "utf8"));
  const names = entries.map((e) => `${e.entry} ${e.runsOn}`);
  assert.ok(names.some((n) => /^sandbox-scenarios ubuntu-/.test(n)), names.join(", "));
  // The macOS scenarios on every pinned version the merge queue requires.
  assert.ok(names.includes("sandbox-scenarios macos-15"), names.join(", "));
  assert.ok(names.includes("sandbox-scenarios macos-26"), names.join(", "));
  assert.ok(names.some((n) => /^existing-macos-sandbox-tests macos-/.test(n)), names.join(", "));
});

test("a line reads as entry, runs-on, timeout and its optional fields", () => {
  assert.deepEqual(parseList(sample), [
    { entry: "placeholder", runsOn: "ubuntu-24.04", timeout: 30, kind: undefined },
    { entry: "refusals-hold", runsOn: "macos-15", timeout: 90, kind: "escape" },
  ]);
});

test("a malformed line is refused with its line number", () => {
  assert.throws(() => parseList("a ubuntu-24.04\n"), /line 1/);
  assert.throws(() => parseList("a ubuntu-24.04 zero\n"), /line 1.*timeout/);
  assert.throws(() => parseList("# c\na ubuntu-24.04 30 kind: elsewhere\n"), /line 2.*kind/);
  // There is no private mirror (Q301): `repo:` is no field.
  assert.throws(() => parseList("a ubuntu-24.04 30 repo: mirror\n"), /line 1.*repo/);
  assert.throws(() => parseList("a ubuntu-24.04 30 colour: red\n"), /line 1.*colour/);
  assert.throws(() => parseList("a ubuntu-24.04 30\na ubuntu-24.04 40\n"), /line 2.*twice/);
  assert.throws(() => parseList("A_B ubuntu-24.04 30\n"), /line 1.*entry/);
});

test("a listed entry becomes one row of the scheduler's matrix, an escape entry marked as one", () => {
  assert.deepEqual(matrixOf(parseList(sample)), {
    include: [
      { entry: "placeholder", "runs-on": "ubuntu-24.04", timeout: 30, escape: false },
      { entry: "refusals-hold", "runs-on": "macos-15", timeout: 90, escape: true },
    ],
  });
});

test("the list's tool has no mirror and no quiet run (Q301)", () => {
  const source = readFileSync(new URL("./nightly-list.mjs", import.meta.url), "utf8");
  assert.doesNotMatch(source, /mirror|quiet|Q287|Q298/i);
  assert.doesNotMatch(readFileSync(LIST, "utf8"), /mirror|quiet/i);
});

test("a harness re-record's ordinary failure is never filed as an escape (T1.1-03)", () => {
  const entries = parseList(readFileSync(LIST, "utf8"));
  const kind = (name) => entries.find((e) => e.entry === name)?.kind;
  // Each re-record has an ordinary entry, red on a failed install, build, recording or `gh`,
  // and an escape entry, red on an escape alone.
  for (const at of ["pinned", "latest"]) {
    assert.equal(kind(`harness-rerecord-${at}`), undefined, `harness-rerecord-${at}`);
    assert.equal(kind(`harness-escapes-${at}`), "escape", `harness-escapes-${at}`);
  }
  const nightly = readFileSync(new URL("../.github/actions/nightly/action.yml", import.meta.url), "utf8");
  for (const [entry, mode] of [
    ["harness-rerecord-pinned", "drift"],
    ["harness-rerecord-latest", "drift"],
    ["harness-escapes-pinned", "escape"],
    ["harness-escapes-latest", "escape"],
  ]) {
    const step = nightly.split("\n    - ").find((s) => s.includes(`inputs.entry == '${entry}'`));
    assert.match(step ?? "", new RegExp(`mode: ${mode}\\b`), entry);
  }
  // In escape mode every step before the recording goes on when it fails.
  const action = readFileSync(new URL("../.github/actions/harness-recordings/action.yml", import.meta.url), "utf8");
  const steps = action.slice(action.indexOf("  steps:"), action.indexOf("    - name: Record each harness"));
  const before = steps.split("\n    - ").slice(1);
  assert.ok(before.length >= 4, steps);
  for (const step of before) {
    assert.match(step, /continue-on-error: \$\{\{ inputs\.mode == 'escape' \}\}/, step.split("\n")[0]);
  }
});
