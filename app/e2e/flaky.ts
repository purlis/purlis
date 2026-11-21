// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { relative } from "node:path";
import { flakyAnnotation } from "../../tools/flake-annotation.mjs";

/** How many times a CI run starts a spec file again after it failed (ADR 0127). */
export const SPEC_FILE_RETRIES = process.env.CI ? 1 : 0;

/**
 * The warnings for the spec files of a worker that has ended: WebdriverIO's `onWorkerEnd` is told
 * how many retries were *left*, so a worker that exited 0 with fewer left than it began with
 * passed on a retry. That is a flake, written in the format every runner writes
 * (`tools/flake-annotation.mjs`), one line per spec file.
 */
export function flakesOfWorker(
  exitCode: number,
  specs: string[],
  retriesLeft: number,
  root: string = process.cwd(),
  allowed: number = SPEC_FILE_RETRIES,
): string[] {
  const retried = allowed - retriesLeft;
  if (exitCode !== 0 || retried <= 0) return [];
  return specs.map((spec) =>
    flakyAnnotation({
      retries: retried,
      test: relative(root, spec.replace(/^file:\/\//, "")).split("\\").join("/"),
    }),
  );
}
