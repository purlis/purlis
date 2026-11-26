// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { skippedIds } from "../../tools/quarantine.mjs";

/**
 * The spec files this run leaves out because `ci/quarantine.toml` lists them (ADR 0127), as
 * `exclude` globs from this folder: `e2e/specs/x.e2e.ts` in the list is `./specs/x.e2e.ts` here.
 * The nightly run, and a person's machine, leave none out.
 */
export function quarantinedSpecs(env: Record<string, string | undefined> = process.env): string[] {
  return skippedIds("wdio", env).map((id) => `./${id.replace(/^e2e\//, "")}`);
}
