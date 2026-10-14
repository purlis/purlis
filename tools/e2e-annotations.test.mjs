// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The scenario job's failed tests as GitHub annotations (`tools/e2e-annotations.mjs`). Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  EVIDENCE_CHARS,
  EVIDENCE_FOR,
  LIMIT,
  MESSAGE_CHARS,
  annotations,
  lineOf,
  readRecords,
} from "./e2e-annotations.mjs";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "e2e-annotations.mjs",
);
const root = "/work/purlis";
const spec = `${root}/app/e2e/specs/tabs.e2e.ts`;
const noSource = () => "";

function failure(overrides = {}) {
  return {
    file: spec,
    parent: "tabs",
    title: "a closed tab is gone",
    message: "Expected 2 tabs, received 3",
    stack: `Error: Expected 2 tabs, received 3\n    at Context.<anonymous> (${spec}:42:11)\n    at node:internal/process/task_queues:95:5`,
    ...overrides,
  };
}

test("a failed test is an error on its spec's line, relative to the repository, with its message", () => {
  assert.deepEqual(annotations([failure()], { root, read: noSource }), [
    "::error file=app/e2e/specs/tabs.e2e.ts,line=42,title=tabs › a closed tab is gone::Expected 2 tabs, received 3",
  ]);
});

test("the line comes from a file:// frame too", () => {
  const record = failure({ stack: `Error: x\n    at file://${spec}:7:3` });
  assert.equal(lineOf(record, root, noSource), 7);
});

test("with no frame in the spec, the line that names the test is used, else 1", () => {
  const timeout = failure({
    stack:
      "Error: Timeout of 180000ms exceeded\n    at listOnTimeout (node:internal/timers:1:1)",
  });
  const source =
    'describe("tabs", () => {\n  it("a closed tab is gone", async () => {\n';
  assert.equal(
    lineOf(timeout, root, () => source),
    2,
  );
  assert.equal(lineOf(timeout, root, noSource), 1);
});

test("colour codes go, a long message is cut, and newlines, % and the property separators are escaped", () => {
  const long = "x".repeat(MESSAGE_CHARS * 2);
  const [cut] = annotations(
    [failure({ message: `\u001b[31m${long}\u001b[39m` })],
    {
      root,
      read: noSource,
    },
  );
  const message = cut.slice(cut.indexOf("::", 2) + 2);
  assert.equal(message.length, MESSAGE_CHARS);
  assert.ok(message.endsWith("…"));

  const [escaped] = annotations(
    [failure({ title: "50%: a, b", message: "line one\nline two 100%" })],
    {
      root,
      read: noSource,
    },
  );
  assert.equal(
    escaped,
    "::error file=app/e2e/specs/tabs.e2e.ts,line=42,title=tabs › 50%25%3A a%2C b::line one%0Aline two 100%25",
  );
});

test("a mass failure is ten errors, ten warnings and a notice counting the rest", () => {
  const records = Array.from({ length: 35 }, (_, i) =>
    failure({ title: `test ${i}` }),
  );
  const lines = annotations(records, { root, read: noSource });
  assert.equal(lines.length, LIMIT + 1);
  assert.equal(lines.filter((l) => l.startsWith("::error ")).length, 10);
  assert.equal(lines.filter((l) => l.startsWith("::warning ")).length, 10);
  assert.match(lines[10], /title=also failed › tabs › test 10::/);
  assert.match(
    lines[LIMIT],
    /^::notice title=\+15 more failed scenario tests::15 more failed; 35 in all\./,
  );
});

test("a job that failed with no test recorded still says so", () => {
  const lines = annotations([], { root, read: noSource });
  assert.equal(lines.length, 1);
  assert.match(
    lines[0],
    /^::error title=scenario tests failed with no test recorded::/,
  );
});

test("a record without a file or a message still annotates", () => {
  assert.deepEqual(
    annotations([{ title: '"before all" hook' }], { root, read: noSource }),
    ['::error title="before all" hook::failed with no message'],
  );
});

test("a half-written line is skipped and the rest are read", () => {
  const text = `${JSON.stringify(failure())}\n{"file": "cut sh\n\n${JSON.stringify(failure({ title: "b" }))}\n`;
  assert.deepEqual(
    readRecords(text).map((r) => r.title),
    ["a closed tab is gone", "b"],
  );
});

test("run as a script, it prints the annotations for a failures file, or the no-test one for none", () => {
  const dir = mkdtempSync(join(tmpdir(), "e2e-annotations-"));
  const file = join(dir, "failures.jsonl");
  writeFileSync(
    file,
    `${JSON.stringify(failure({ file: "app/e2e/specs/tabs.e2e.ts", stack: "" }))}\n`,
  );
  const run = spawnSync(process.execPath, [script, file], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
  assert.match(
    run.stdout,
    /^::error file=app\/e2e\/specs\/tabs\.e2e\.ts,line=\d+,title=tabs › a closed tab is gone::/,
  );

  const none = spawnSync(
    process.execPath,
    [script, join(dir, "missing.jsonl")],
    {
      encoding: "utf8",
    },
  );
  assert.equal(none.status, 0, none.stderr);
  assert.match(none.stdout, /no test recorded/);
});

test("the first failures carry what the window was still waiting for and the core's last log lines", () => {
  // #1949's scenario stall: the job's log and artifact are not what a reader of the checks has,
  // so the failure itself says which command never answered and what the core said last.
  const record = failure({
    late: [
      { command: "createProject", ms: null },
      { command: "openPlane", ms: 41000 },
    ],
    coreLog: ["purlis: one line", "\u001b[31mpurlis: a red line\u001b[0m"],
  });
  const [line] = annotations([record], { root, read: noSource });
  const message = line.slice(line.indexOf("::", 2) + 2);
  assert.ok(message.startsWith("Expected 2 tabs, received 3%0A%0A"), message);
  assert.ok(
    message.includes(
      "the window waited for: createProject (never answered), openPlane (answered after 41 s)",
    ),
    message,
  );
  assert.ok(
    message.includes(
      "the core's last log lines:%0Apurlis: one line%0Apurlis: a red line",
    ),
    message,
  );
});

test("only the first few failures carry the evidence, each held to its own size", () => {
  const long = Array.from(
    { length: 400 },
    (_, n) => `purlis: line ${n} ${"x".repeat(60)}`,
  );
  const records = Array.from({ length: EVIDENCE_FOR + 2 }, (_, n) =>
    failure({ title: `test ${n}`, coreLog: long }),
  );
  const lines = annotations(records, { root, read: noSource });
  for (const [n, line] of lines.entries()) {
    const has = line.includes("the core's last log lines");
    assert.equal(has, n < EVIDENCE_FOR, `annotation ${n}`);
    assert.ok(
      line.length < MESSAGE_CHARS + EVIDENCE_CHARS + 400,
      `annotation ${n} is too long`,
    );
  }
});

test("a failure that carried an empty block says so, and one that carried none says nothing", () => {
  // A quiet core and a block that was never recorded read differently (#2029's review).
  const [quiet] = annotations([failure({ late: [], coreLog: [] })], {
    root,
    read: noSource,
  });
  assert.ok(
    quiet.includes("no command was late; the core logged nothing"),
    quiet,
  );
  const [none] = annotations([failure()], { root, read: noSource });
  assert.ok(!none.includes("%0A%0A"), none);
});

test("only failures that carried a block count towards the first few that show it", () => {
  // The left-chats guard writes a record of its own, with no block: it does not use up a place.
  const records = [
    failure({ title: "first", late: [], coreLog: ["purlis: one"] }),
    failure({ title: "the guard's own record" }),
    ...Array.from({ length: EVIDENCE_FOR }, (_, n) =>
      failure({ title: `after ${n}`, late: [], coreLog: ["purlis: one"] }),
    ),
  ];
  const lines = annotations(records, { root, read: noSource });
  const shown = lines.filter((line) =>
    line.includes("the core's last log lines"),
  ).length;
  assert.equal(shown, EVIDENCE_FOR);
  assert.ok(EVIDENCE_FOR >= 5);
});

test("what covered the element a test could not press is said", () => {
  const record = failure({
    message: 'element ("button=New tab") still not clickable after 20000ms',
    late: [],
    coreLog: [],
    covers: [
      "at its centre: div.palette-backdrop",
      "focused: input#palette",
      "open: [role=dialog] Switch project",
    ],
  });
  const [line] = annotations([record], { root, read: noSource });
  assert.ok(
    line.includes(
      "what covered it:%0Aat its centre: div.palette-backdrop%0Afocused: input#palette%0Aopen: [role=dialog] Switch project",
    ),
    line,
  );
});
