// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The one annotation every runner writes for a flake (spec #7324, ADR 0127): a test that failed,
// then passed on its retry in the same run. nextest's path (`tools/slowest-tests.mjs`), vitest's
// reporter (`tools/vitest-flaky-reporter.mjs`) and wdio's `onWorkerEnd` (`app/e2e/flaky.ts`) all
// write it here, so `tools/ci-health.mjs` reads one title for all three: `Flaky (passed on retry
// N): <test>`.

import { escapeData, escapeProperty } from "./e2e-annotations.mjs";

/** The title ci-health's reader matches (`FLAKY` in `tools/ci-health.mjs`). */
export const flakyTitle = (retries, test) => `Flaky (passed on retry ${retries}): ${test}`;

/**
 * The `::warning` line for one flake: `retries` is how many failed attempts came before the pass,
 * `test` is the id ci-health counts it by, `said` the lines its failed attempt wrote.
 */
export function flakyAnnotation({ retries, test, said = [] }) {
  const lines = said.length > 0 ? said : ["failed"];
  return `::warning title=${escapeProperty(flakyTitle(retries, test))}::${escapeData(lines.join("\n"))}`;
}
