// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The nightly list (`.github/nightly-on-<branch>.txt`, T1.1-09): main's scheduler checks out the
// branch and turns the list into its matrix with `nightly-list.mjs`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { matrixOf, parseList } from "./nightly-list.mjs";

const LIST = new URL("../.github/nightly-on-phase-1-proven.txt", import.meta.url);
const PUBLIC = { repository: "purlis/purlis", mirror: "" };

const sample = `# entry   runs-on   timeout   [repo: mirror] [kind: escape]
placeholder        ubuntu-24.04   30

fuzz-parsers       ubuntu-24.04   60   repo: mirror
refusals-hold      macos-15       90   kind: escape
`;

test("the nightly list on the branch is well formed and lists the placeholder entries", () => {
  const entries = parseList(readFileSync(LIST, "utf8"));
  const names = entries.map((e) => `${e.entry} ${e.runsOn}`);
  assert.ok(names.some((n) => /^sandbox-scenarios ubuntu-/.test(n)), names.join(", "));
  assert.ok(names.some((n) => /^sandbox-scenarios macos-/.test(n)), names.join(", "));
  assert.ok(names.some((n) => /^existing-macos-sandbox-tests macos-/.test(n)), names.join(", "));
});

test("a line reads as entry, runs-on, timeout and its optional fields", () => {
  assert.deepEqual(parseList(sample), [
    { entry: "placeholder", runsOn: "ubuntu-24.04", timeout: 30, repo: undefined, kind: undefined },
    { entry: "fuzz-parsers", runsOn: "ubuntu-24.04", timeout: 60, repo: "mirror", kind: undefined },
    { entry: "refusals-hold", runsOn: "macos-15", timeout: 90, repo: undefined, kind: "escape" },
  ]);
});

test("a malformed line is refused with its line number", () => {
  assert.throws(() => parseList("a ubuntu-24.04\n"), /line 1/);
  assert.throws(() => parseList("a ubuntu-24.04 zero\n"), /line 1.*timeout/);
  assert.throws(() => parseList("# c\na ubuntu-24.04 30 repo: elsewhere\n"), /line 2.*repo/);
  assert.throws(() => parseList("a ubuntu-24.04 30 colour: red\n"), /line 1.*colour/);
  assert.throws(() => parseList("a ubuntu-24.04 30\na ubuntu-24.04 40\n"), /line 2.*twice/);
  assert.throws(() => parseList("A_B ubuntu-24.04 30\n"), /line 1.*entry/);
});

test("a listed entry becomes one row of the scheduler's matrix", () => {
  const { include } = matrixOf(parseList(sample), PUBLIC).matrix;
  assert.deepEqual(include[0], { entry: "placeholder", "runs-on": "ubuntu-24.04", timeout: 30, quiet: false });
});

test("a_repo_mirror_entry_is_skipped_on_the_public_repo", () => {
  const onPublic = matrixOf(parseList(sample), PUBLIC);
  assert.deepEqual(
    onPublic.matrix.include.map((e) => e.entry),
    ["placeholder", "refusals-hold"],
  );
  assert.deepEqual(onPublic.skipped, ["fuzz-parsers"]);
  // With no mirror named, no repository is the mirror.
  assert.deepEqual(matrixOf(parseList(sample), { repository: "", mirror: "" }).skipped, ["fuzz-parsers"]);
  const onMirror = matrixOf(parseList(sample), { repository: "purlis/mirror", mirror: "purlis/mirror" });
  assert.deepEqual(onMirror.skipped, []);
  assert.equal(onMirror.matrix.include.length, 3);
});

test("an escape entry on the public repo runs quiet, and loud on the mirror", () => {
  const quiet = (where) =>
    Object.fromEntries(matrixOf(parseList(sample), where).matrix.include.map((e) => [e.entry, e.quiet]));
  assert.deepEqual(quiet(PUBLIC), { placeholder: false, "refusals-hold": true });
  assert.equal(quiet({ repository: "purlis/mirror", mirror: "purlis/mirror" })["refusals-hold"], false);
});
