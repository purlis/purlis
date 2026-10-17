// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What the relaunch test (`tools/relaunch.mjs`) reads and reports: the stand-in harness's lines,
// the steps a launch took, and a failure as GitHub annotations (`tools/relaunch-report.mjs`).
// Run with `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DETAIL_CHARS,
  annotations,
  checkLine,
  linesWritten,
  stepTimer,
  stopLine,
} from "./relaunch-report.mjs";

test("a line the stand-in harness is still writing is not a start yet (#2183)", () => {
  // Its arguments run to some 190 KB, written in more than one piece: a read between two pieces
  // sees the line's head, without the `--resume` at its end.
  assert.deepEqual(linesWritten("--resume a --name one\n--plugin-dir /x --settings {"), [
    "--resume a --name one",
  ]);
});

test("every finished line is one start, and nothing written is none", () => {
  assert.deepEqual(linesWritten("one\ntwo\n"), ["one", "two"]);
  assert.deepEqual(linesWritten(""), []);
});

/** A clock the test moves by hand. */
function aClock(start = 1_000) {
  let now = start;
  return { now: () => now, pass: (ms) => (now += ms) };
}

test("the step timer says each step a launch took and how long, and the one it is in", () => {
  const clock = aClock();
  const steps = stepTimer(clock.now);
  steps.step("starting the app");
  clock.pass(400);
  steps.step("waiting for the launch's question");
  clock.pass(61_250);
  steps.step("waiting for the chat to start");
  clock.pass(28_350);
  assert.equal(steps.current(), "waiting for the chat to start");
  assert.equal(
    steps.summary(),
    "starting the app 0.4 s, waiting for the launch's question 61.3 s, " +
      "waiting for the chat to start 28.4 s",
  );
});

test("a timer no step was started on says so", () => {
  const steps = stepTimer(aClock().now);
  assert.equal(steps.current(), undefined);
  assert.equal(steps.summary(), "no step started");
});

function aFailure(overrides = {}) {
  return {
    launch: "the fourth launch, after a restart to update",
    what: "and Reopen all resumes the chat the restart recorded",
    detail: 'it was given: "--plugin-dir /p --name ide.7"',
    steps: "starting the app 0.1 s, waiting for the chat to start 0.4 s",
    ...overrides,
  };
}

test("a failed check is an error titled with its launch and its words, saying what it saw and the steps", () => {
  assert.deepEqual(annotations([aFailure()]), [
    "::error title=relaunch › the fourth launch%2C after a restart to update › " +
      "and Reopen all resumes the chat the restart recorded::" +
      'it was given: "--plugin-dir /p --name ide.7"%0A%0A' +
      "steps: starting the app 0.1 s, waiting for the chat to start 0.4 s",
  ]);
});

test("a long detail keeps its head and its end, where an argument list's `--resume` is", () => {
  const detail = `it was given: "--plugin-dir ${"x".repeat(200_000)} --resume abc --name ide.7"`;
  const [line] = annotations([aFailure({ detail, steps: "" })]);
  const message = line.slice(line.indexOf("::", 2) + 2);
  assert.ok(message.length <= DETAIL_CHARS, `${message.length} characters`);
  assert.ok(message.startsWith('it was given: "--plugin-dir xxx'), message.slice(0, 40));
  assert.ok(message.endsWith('--resume abc --name ide.7"'), message.slice(-40));
  assert.ok(message.includes("…"));
});

test("a line of what it saw that looks like it holds a secret is not written out", () => {
  // Put together here, so the repository's own secret scan does not take the fixture for one.
  const token = ["ghp", "_", "abcdefghijklmnopqrstuvwxyz"].join("");
  const [line] = annotations([
    aFailure({ detail: `the app said:\nrunning on ${token}\nthen it quit`, steps: "" }),
  ]);
  assert.ok(!line.includes(token), line);
  assert.ok(line.includes("then it quit"), line);
  assert.ok(line.includes("[redacted: "), line);
});

test("past ten failures, a notice counts the rest, since GitHub keeps ten errors a step", () => {
  const failures = Array.from({ length: 13 }, (_, n) => aFailure({ what: `check ${n}` }));
  const lines = annotations(failures);
  assert.equal(lines.length, 11);
  assert.ok(lines.slice(0, 10).every((line) => line.startsWith("::error ")));
  assert.ok(lines[10].startsWith("::notice title=+3 more failed relaunch checks::"), lines[10]);
});

test("no failure, no annotation", () => {
  assert.deepEqual(annotations([]), []);
});

test("a token across the cut is redacted whole, not left as a fragment the shapes miss", () => {
  const token = ["ghp", "_", "abcdefghijklmnopqrstuvwxyz0123"].join("");
  // The head the cut keeps ends a few characters into the token.
  const head = Math.floor((DETAIL_CHARS - 1) / 2);
  const detail = `${"a".repeat(head - 8)} ${token} ${"b".repeat(DETAIL_CHARS * 2)}`;
  const [line] = annotations([aFailure({ detail, steps: "" })]);
  assert.ok(!line.includes("ghp_"), line.slice(0, head + 20));
});

test("what it saw is annotated without colour codes", () => {
  const [line] = annotations([aFailure({ detail: "\u001b[31mExpected\u001b[39m 2", steps: "" })]);
  assert.ok(line.endsWith("::Expected 2"), line);
});

test("a stopped timer's last step ends when it was stopped, however late it is read", () => {
  const clock = aClock();
  const steps = stepTimer(clock.now);
  steps.step("ending the app");
  clock.pass(3_000);
  steps.stop();
  clock.pass(60_000);
  // An answer still trying in the background, after the launch was ended, adds nothing.
  steps.step("waiting for the launch's question");
  assert.equal(steps.summary(), "ending the app 3.0 s");
});

/** A token-shaped value, put together here so the repository's own secret scan does not take it
 *  for one. */
const aToken = () => ["ghp", "_", "abcdefghijklmnopqrstuvwxyz0123"].join("");

test("a check's line in the job log says what it saw, with a line that looks like a secret not written out (#2382)", () => {
  const line = checkLine(
    "the app starts the chat",
    false,
    `the app said:\nrunning on ${aToken()}\nthen it quit`,
  );
  assert.equal(
    line,
    "FAIL the app starts the chat — the app said:\n" +
      "[redacted: a line that looks like it holds a GitHub token]\nthen it quit",
  );
});

test("a check that passed, or saw nothing, is one plain line", () => {
  assert.equal(
    checkLine("start fresh clears the record", true),
    "ok   start fresh clears the record",
  );
  assert.equal(checkLine("and starts no chat", false, ""), "FAIL and starts no chat");
});

test("a run that stopped prints its stack, with a line that looks like a secret not written out (#2382)", () => {
  const error = new Error(`POST /session: refused for ${aToken()}`);
  const printed = stopLine(error);
  assert.ok(!printed.includes(aToken()), printed);
  assert.ok(printed.includes("[redacted: "), printed);
  assert.ok(printed.includes("relaunch-report.test.mjs"), printed);
});

test("a run that stopped on something that is not an Error says what it was", () => {
  assert.equal(stopLine("the record is not JSON"), "the record is not JSON");
});
