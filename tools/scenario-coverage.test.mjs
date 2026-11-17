// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The coverage list's generator and re-carry diff (T1.1-08), on a small matrix written here.

import assert from "node:assert/strict";
import { test } from "node:test";

import { carried, diff, readMatrix } from "./scenario-coverage.mjs";

const MATRIX = `
### 3.1 Network blocks

| ID | Steps |
|---|---|
| M-NET-01 | a GET |
| M-2CH-01 | two chats |

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
| M-2CH-01 | all | same | same | both | e2e: the window's DOM only (ticket 6 leaves it to e2e) | ·✓· |

## 4. Already known
| M-NET-99 | not a §3 row |
`;

test("the matrix's rows, sections, covers and fixture rows are read as written", () => {
  const { section, cover, fixtures } = readMatrix(MATRIX);
  assert.deepEqual(section, { "M-NET-01": "3.1", "M-2CH-01": "3.1" });
  assert.deepEqual(Object.keys(cover), ["M-NET-01", "M-2CH-01"]);
  assert.deepEqual(fixtures, ["M-OBS-11", "M-ATT-01", "M-ATT-02", "R-006", "R-008"]);
  assert.deepEqual(carried(cover["M-2CH-01"]), { kind: "e2e", reason: "window" });
  assert.throws(() => carried("by hand"), /does not know/);
});

test("a re-carry that adds a row, drops one or moves its cover kind is a difference", () => {
  const list = {
    rows: [
      { row: "M-NET-01", carried: "scenario" },
      { row: "M-2CH-01", carried: "e2e" },
    ],
    fixtures: ["M-OBS-11", "M-ATT-01", "M-ATT-02", "R-006", "R-008"].map((row) => ({ row })),
  };
  assert.deepEqual(diff(MATRIX, list), []);

  const moved = { ...list, rows: [{ row: "M-NET-01", carried: "fixture" }, list.rows[1]] };
  assert.match(diff(MATRIX, moved).join("\n"), /M-NET-01: the matrix carries scenario, the list fixture/);

  const added = MATRIX.replace("| M-2CH-01 | all |", "| M-NET-02 | all | same | same | both | scenario: x | ✓·· |\n| M-2CH-01 | all |");
  assert.match(diff(added, list).join("\n"), /M-NET-02: in the matrix's §3\.22, not in the list/);

  const dropped = { ...list, fixtures: list.fixtures.slice(1) };
  assert.match(diff(MATRIX, dropped).join("\n"), /M-OBS-11: in the matrix's §3\.20, not in the list's fixtures/);
});
