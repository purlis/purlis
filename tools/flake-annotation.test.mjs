// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { test } from "node:test";

import { flakyTests } from "./ci-health.mjs";
import { flakyAnnotation } from "./flake-annotation.mjs";
import { flakyAnnotations } from "./slowest-tests.mjs";

/** What GitHub's checks API answers for a `::warning title=…::…` line. */
function asRecorded(line) {
  const [, title] = /^::warning title=([^:]*)::/.exec(line);
  return decodeURIComponent(title);
}
const suiteOf = (...lines) => ({
  checkRuns: {
    nodes: [{ name: "job", conclusion: "SUCCESS", annotations: { nodes: lines.map((line) => ({ title: asRecorded(line) })) } }],
  },
});

test("a flake of any runner is a warning titled with its retry count and its test", () => {
  const line = flakyAnnotation({ retries: 1, test: "src/a.test.ts > a thing > works", said: ["boom"] });
  assert.equal(line, "::warning title=Flaky (passed on retry 1)%3A src/a.test.ts > a thing > works::boom");
});

test("a flake that said nothing is still told it failed", () => {
  assert.ok(flakyAnnotation({ retries: 1, test: "x" }).endsWith("::failed"));
});

test("ci-health counts the flakes of nextest, vitest and wdio alike", () => {
  const rust = flakyAnnotations([{ binary: "purlis-core::it", name: "a_test", retries: 1, said: ["x"] }]);
  const vitest = [flakyAnnotation({ retries: 1, test: "src/a.test.ts > suite > b" })];
  const wdio = [flakyAnnotation({ retries: 1, test: "specs/onboarding.e2e.ts" })];
  const counted = flakyTests([suiteOf(...rust), suiteOf(...vitest), suiteOf(...wdio), suiteOf(...wdio)]);
  assert.deepEqual(counted, [
    { test: "specs/onboarding.e2e.ts", count: 2 },
    { test: "purlis-core::it a_test", count: 1 },
    { test: "src/a.test.ts > suite > b", count: 1 },
  ]);
});
