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
