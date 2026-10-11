// Which scenario specs and tests took longest, read from `app/logs/timings.jsonl`. Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { annotations, readTimings } from "./e2e-timings.mjs";

/** What `app/e2e/timings.ts` writes: a line per test, and a line per spec file's session. */
const jsonl = [
  {
    at: 1,
    kind: "test",
    file: "/runner/work/purlis/app/e2e/specs/opening.e2e.ts",
    parent: "Opening",
    title: "a repo",
    ms: 41_000,
  },
  {
    at: 2,
    kind: "test",
    file: "/runner/work/purlis/app/e2e/specs/opening.e2e.ts",
    parent: "Opening",
    title: "a gitlab repo",
    ms: 9_500,
  },
  {
    at: 3,
    kind: "spec",
    file: "file:///runner/work/purlis/app/e2e/specs/opening.e2e.ts",
    ms: 55_250,
  },
  "{ cut short by a kill",
  {
    at: 4,
    kind: "test",
    file: "/runner/work/purlis/app/e2e/specs/vaults.e2e.ts",
    parent: "Vaults",
    title: "reveal",
    ms: 12_000,
  },
  {
    at: 5,
    kind: "spec",
    file: "/runner/work/purlis/app/e2e/specs/vaults.e2e.ts",
    ms: 13_000,
  },
]
  .map((line) => (typeof line === "string" ? line : JSON.stringify(line)))
  .join("\n");

test("tests and spec files are read, a line that is not JSON is skipped, paths from e2e/ on", () => {
  const { tests, specs } = readTimings(jsonl);
  assert.deepEqual(
    tests.map((one) => [one.file, one.name, one.seconds]),
    [
      ["e2e/specs/opening.e2e.ts", "Opening › a repo", 41],
      ["e2e/specs/opening.e2e.ts", "Opening › a gitlab repo", 9.5],
      ["e2e/specs/vaults.e2e.ts", "Vaults › reveal", 12],
    ],
  );
  assert.deepEqual(specs, [
    { file: "e2e/specs/opening.e2e.ts", seconds: 55.25 },
    { file: "e2e/specs/vaults.e2e.ts", seconds: 13 },
  ]);
});

test("two notices, slowest first: the spec files by their session's time, and the tests", () => {
  const lines = annotations(readTimings(jsonl), {
    label: "macos-latest, part 1 of 2",
  });
  assert.equal(lines.length, 2);
  assert.match(
    lines[0],
    /^::notice title=Slowest scenario spec files \(macos-latest%2C part 1 of 2\)::/,
  );
  assert.match(
    lines[0],
    /55\.3 s {2}e2e\/specs\/opening\.e2e\.ts%0A *13\.0 s {2}e2e\/specs\/vaults/,
  );
  assert.match(
    lines[1],
    /^::notice title=Slowest scenario tests \(macos-latest%2C part 1 of 2\)::/,
  );
  assert.match(
    lines[1],
    /41\.0 s {2}e2e\/specs\/opening\.e2e\.ts {2}Opening › a repo%0A *12\.0 s/,
  );
});

test("a run that recorded nothing says so", () => {
  assert.deepEqual(annotations(readTimings(""), { label: "ubuntu-24.04" }), [
    "::notice title=Slowest scenario spec files (ubuntu-24.04)::No scenario timings were recorded.",
  ]);
});
