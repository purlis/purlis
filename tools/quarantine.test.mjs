// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { QUARANTINE_FILE, nextestFilter, parseQuarantine, problemsWith, runnerOf, skippedIds, skipsQuarantine } from "./quarantine.mjs";

const LIST = `# a comment
[[test]]
id = "purlis-core::it a_flaky_test"  # why
ticket = 7001
added = "2026-11-21"

[[test]]
id = "src/Thing.test.ts > a suite > a flaky test"
ticket = 7002
added = "2026-11-20"

[[test]]
id = "e2e/specs/thing.e2e.ts"
ticket = 7003
added = "2026-11-19"
`;

/** A quarantine file of `text`, named by the environment the way a run names a fixture's. */
function listed(text, env) {
  const file = join(mkdtempSync(join(tmpdir(), "quarantine-")), "quarantine.toml");
  writeFileSync(file, text);
  return { PURLIS_QUARANTINE_FILE: file, ...env };
}
const PR = { CI: "true", GITHUB_EVENT_NAME: "pull_request" };

test("an entry is its test's id, its ticket and the day it was added", () => {
  assert.deepEqual(parseQuarantine(LIST)[0], { id: "purlis-core::it a_flaky_test", ticket: 7001, added: "2026-11-21" });
  assert.equal(parseQuarantine(LIST).length, 3);
});

test("a file with nothing in it quarantines nothing", () => {
  assert.deepEqual(parseQuarantine("# empty\n"), []);
  assert.deepEqual(problemsWith([]), []);
});

test("an entry without a ticket or a date is refused", () => {
  const [noTicket, noDate] = [
    parseQuarantine('[[test]]\nid = "a b"\nadded = "2026-11-21"\n'),
    parseQuarantine('[[test]]\nid = "a b"\nticket = 7001\n'),
  ];
  assert.match(problemsWith(noTicket).join("\n"), /"a b" has no ticket/);
  assert.match(problemsWith(noDate).join("\n"), /"a b" has no date/);
  assert.match(problemsWith(parseQuarantine('[[test]]\nid = "a b"\nticket = 7001\nadded = "soon"\n')).join("\n"), /no date/);
});

test("a test listed twice is refused", () => {
  const twice = parseQuarantine('[[test]]\nid = "a b"\nticket = 1\nadded = "2026-11-21"\n'.repeat(2));
  assert.match(problemsWith(twice).join("\n"), /listed twice/);
});

test("a line the reader does not know is refused rather than skipped", () => {
  assert.throws(() => parseQuarantine("[[test]]\nid = 'single quotes'\n"), /line 2/);
  assert.throws(() => parseQuarantine('id = "outside an entry"\n'), /line 1/);
});

test("the checked-in list is valid", () => {
  assert.deepEqual(problemsWith(parseQuarantine(readFileSync(QUARANTINE_FILE, "utf8"))), []);
});

test("each runner's id is told by its shape", () => {
  assert.equal(runnerOf("purlis-core::it a_flaky_test"), "nextest");
  assert.equal(runnerOf("src/Thing.test.ts > a suite > a flaky test"), "vitest");
  assert.equal(runnerOf("e2e/specs/thing.e2e.ts"), "wdio");
});

test("pull request, queue and main runs skip the quarantine, and the nightly run and a person's machine do not", () => {
  for (const event of ["pull_request", "merge_group", "push"]) assert.equal(skipsQuarantine({ CI: "true", GITHUB_EVENT_NAME: event }), true, event);
  assert.equal(skipsQuarantine({ CI: "true", GITHUB_EVENT_NAME: "schedule" }), false);
  assert.equal(skipsQuarantine({}), false);
});

test("each runner skips its own quarantined test under a pull request run and runs it under the nightly", () => {
  const env = (extra) => listed(LIST, extra);
  assert.deepEqual(skippedIds("nextest", env(PR)), ["purlis-core::it a_flaky_test"]);
  assert.deepEqual(skippedIds("vitest", env(PR)), ["src/Thing.test.ts > a suite > a flaky test"]);
  assert.deepEqual(skippedIds("wdio", env(PR)), ["e2e/specs/thing.e2e.ts"]);
  for (const runner of ["nextest", "vitest", "wdio"]) {
    assert.deepEqual(skippedIds(runner, env({ CI: "true", GITHUB_EVENT_NAME: "schedule" })), [], runner);
  }
});

test("nextest's filterset leaves out each quarantined test by binary and name, and is all() when none", () => {
  assert.equal(nextestFilter(listed(LIST, PR)), "not ((binary_id(=purlis-core::it) & test(=a_flaky_test)))");
  assert.equal(nextestFilter(listed("", PR)), "all()");
  assert.equal(nextestFilter(listed(LIST, { CI: "true", GITHUB_EVENT_NAME: "schedule" })), "all()");
});
