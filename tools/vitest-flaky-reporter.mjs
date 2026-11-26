// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// vitest's flakes, as the annotation every runner writes (`tools/flake-annotation.mjs`).
//
// `app/vite.config.ts` gives vitest `retry: 1` when `CI` is set and this reporter beside the
// default one: a test that failed and then passed on its retry keeps the run green and is named
// as a `Flaky (passed on retry N): <file> > <test>` warning, so it is counted by ci-health and
// never silently green (ADR 0127). A test that fails every attempt is not a flake: vitest fails it.

import { relative } from "node:path";
import process from "node:process";

import { flakyAnnotation } from "./flake-annotation.mjs";

/** The warning for `testCase` (vitest's `TestCase`) when it passed on a retry, else undefined. */
export function flakeOf(testCase, root = process.cwd()) {
  const result = testCase.result();
  const diagnostic = testCase.diagnostic();
  if (result.state !== "passed" || !diagnostic || !(diagnostic.retryCount > 0)) return undefined;
  const file = relative(root, testCase.module.moduleId).split("\\").join("/");
  return flakyAnnotation({ retries: diagnostic.retryCount, test: `${file} > ${testCase.fullName}` });
}

export default class FlakyReporter {
  constructor(options = {}) {
    this.write = options.write ?? ((line) => console.log(line));
    this.root = options.root ?? process.cwd();
  }

  onTestCaseResult(testCase) {
    const line = flakeOf(testCase, this.root);
    if (line) this.write(line);
  }
}
