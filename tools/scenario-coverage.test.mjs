// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The coverage list's generator and re-carry diff (T1.1-08), on a small matrix written here.

import assert from "node:assert/strict";
import { test } from "node:test";

import { mkdtempSync, readFileSync, rmSync, statSync, utimesSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { carried, diff, generate, readMatrix, writeIssues } from "./scenario-coverage.mjs";

const MATRIX = `
### 3.1 Network blocks

| ID | Steps |
|---|---|
| M-NET-01 | a GET |
| M-2CH-01 | two chats |
| M-NET-02 | a GET, under both binaries |

### 3.19 Policy

| ID | What | Need |
|---|---|---|
| M-POL-01…02 | policy-locked labels | F: the policy file is machine-wide |

### 3.20 Left to fixtures (batch **F**)

| ID | What | Why not live |
|---|---|---|
| M-OBS-11 | the Blocked log keeps 30 days | time |
| M-ATT-01…02 | the briefing reads attended | no attendance wording |
| — | R-006, R-008 | unit-only rows |

### 3.22 Every row per dialect and OS, and its cover kind

| Row | H | Codex | OpenCode | OS | Cover | L W C |
|---|---|---|---|---|---|---|
| M-NET-01 | all | same | same | both | scenario: the fake harness under the scenario host (tickets 4, 6, 7) | ✓✓✓ |
| M-NET-02 | all | same | same | both | scenario, and ticket 53's real-binary leg (tickets 4, 6, 7) | ✓·· |
| M-2CH-01 | all | same | same | both | e2e: the window's DOM only (ticket 6 leaves it to e2e) | ·✓· |

## 4. Already known
| M-NET-99 | not a §3 row |
`;

test("the matrix's rows, sections, covers and fixture rows are read as written", () => {
  const { section, cover, fixtures } = readMatrix(MATRIX);
  assert.deepEqual(section, { "M-NET-01": "3.1", "M-2CH-01": "3.1", "M-NET-02": "3.1", "M-POL-01": "3.19", "M-POL-02": "3.19" });
  assert.deepEqual(Object.keys(cover), ["M-NET-01", "M-NET-02", "M-2CH-01"]);
  assert.deepEqual(fixtures, ["M-POL-01", "M-POL-02", "M-OBS-11", "M-ATT-01", "M-ATT-02", "R-006", "R-008"]);
  assert.deepEqual(carried(cover["M-NET-02"]), { kind: "scenario", binary: "both" });
  assert.deepEqual(carried(cover["M-2CH-01"]), { kind: "e2e", reason: "window" });
  assert.throws(() => carried("by hand"), /does not know/);
});

const LIST = {
  rows: [
    { row: "M-NET-01", dialect: "codex", os: "linux", carried: "scenario" },
    { row: "M-NET-02", dialect: "codex", os: "linux", carried: "scenario", binary: "both" },
    { row: "M-2CH-01", dialect: "codex", os: "linux", carried: "e2e" },
  ],
  fixtures: ["M-POL-01", "M-POL-02", "M-OBS-11", "M-ATT-01", "M-ATT-02", "R-006", "R-008"].map((row) => ({ row })),
};

test("a re-carry that adds a row, drops one or moves its cover kind is a difference", () => {
  assert.deepEqual(diff(MATRIX, LIST), []);

  const moved = { ...LIST, rows: [{ ...LIST.rows[0], carried: "fixture" }, ...LIST.rows.slice(1)] };
  assert.match(diff(MATRIX, moved).join("\n"), /M-NET-01 codex linux: the matrix carries scenario, the list fixture/);

  const added = MATRIX.replace("| M-2CH-01 | all |", "| M-NET-03 | all | same | same | both | scenario: x | ✓·· |\n| M-2CH-01 | all |");
  assert.match(diff(added, LIST).join("\n"), /M-NET-03: in the matrix's §3\.22, not in the list/);

  const dropped = { ...LIST, fixtures: LIST.fixtures.filter((f) => f.row !== "M-OBS-11") };
  assert.match(diff(MATRIX, dropped).join("\n"), /M-OBS-11: the matrix leaves it to fixtures, and the list's fixtures don't name it/);
});

test("a row run under both binaries that the list runs under the fake one alone is a difference", () => {
  const fake = { ...LIST, rows: LIST.rows.map((r) => (r.row === "M-NET-02" ? { ...r, binary: "fake" } : r)) };
  assert.match(diff(MATRIX, fake).join("\n"), /M-NET-02 codex linux: the matrix runs it under both binaries, the list fake/);
});

test("a §3 row that §3.22 doesn't carry and the matrix doesn't leave to fixtures fails diff and generate", () => {
  const unneeded = MATRIX.replace("F: the policy file is machine-wide", "P");
  const policy = { ...LIST, fixtures: LIST.fixtures.filter((f) => !f.row.startsWith("M-POL")) };
  assert.match(diff(unneeded, policy).join("\n"), /M-POL-01: a §3\.19 row that §3\.22 doesn't carry/);
  assert.throws(() => generate(unneeded, [], { rows: [] }), /M-POL-02: a §3\.19 row that §3\.22 doesn't carry/);

  const policyDropped = { ...LIST, fixtures: policy.fixtures };
  assert.match(diff(MATRIX, policyDropped).join("\n"), /M-POL-01: the matrix leaves it to fixtures/);
});

test("generate lists the rows a §3 table's Need leaves to fixtures, with their reason", () => {
  const { list } = generate(MATRIX.replace(/\| M-ATT-01…02 .*\n/, "").replace("| — | R-006, R-008 | unit-only rows |\n", ""), [], { rows: [] });
  assert.deepEqual(
    list.fixtures.filter((f) => f.row.startsWith("M-POL")).map((f) => [f.row, f.reason]),
    [["M-POL-01", "machine_wide_state"], ["M-POL-02", "machine_wide_state"]],
  );
});

test("the_issues_cache_is_not_rewritten_when_only_its_day_changed", () => {
  // Cargo takes every file of a path package for its source: a rewrite builds purlis-app's tests again.
  const dir = mkdtempSync(join(tmpdir(), "issues-"));
  const file = join(dir, "issues.json");
  const doc = (fetched, state) => ({ $comment: "c", fetched, issues: { 6674: { state, opened: "2026-11-12T10:00:00Z" } } });
  try {
    assert.equal(writeIssues(file, doc("2026-11-17", "open")), true, "a new file is written");
    const long_ago = new Date("2026-11-17T00:00:00Z");
    utimesSync(file, long_ago, long_ago);
    assert.equal(writeIssues(file, doc("2026-11-18", "open")), false, "the same issues, a new day: nothing written");
    assert.equal(statSync(file).mtime.getTime(), long_ago.getTime(), "its time is untouched");
    assert.match(readFileSync(file, "utf8"), /2026-11-17/);
    assert.equal(writeIssues(file, doc("2026-11-18", "closed")), true, "an issue that closed is written");
    assert.match(readFileSync(file, "utf8"), /closed/);
  } finally {
    rmSync(dir, { recursive: true });
  }
});
