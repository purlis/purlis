// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { test } from "node:test";

import Reporter from "./vitest-flaky-reporter.mjs";

/** The shape of vitest's `TestCase` the reporter reads. */
const testCase = (state, retryCount) => ({
  fullName: "a suite > works",
  module: { moduleId: "/repo/app/src/a.test.ts" },
  result: () => ({ state }),
  diagnostic: () => ({ retryCount, flaky: state === "passed" && retryCount > 0 }),
});

function written(...cases) {
  const lines = [];
  const reporter = new Reporter({ root: "/repo/app", write: (line) => lines.push(line) });
  for (const one of cases) reporter.onTestCaseResult(one);
  return lines;
}

test("a test that failed once and passed on its retry is named as a flake", () => {
  assert.deepEqual(written(testCase("passed", 1)), [
    "::warning title=Flaky (passed on retry 1)%3A src/a.test.ts > a suite > works::failed",
  ]);
});

test("a test that passed first time says nothing", () => {
  assert.deepEqual(written(testCase("passed", 0)), []);
});

test("a test that failed every attempt is not a flake: vitest fails it", () => {
  assert.deepEqual(written(testCase("failed", 1)), []);
});

test("a skipped test says nothing", () => {
  assert.deepEqual(written({ ...testCase("skipped", 0), diagnostic: () => undefined }), []);
});
