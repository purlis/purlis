// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { skippedIds } from "../../tools/quarantine.mjs";

/** The parts of vitest's running test the quarantine reads. */
interface Running {
  name: string;
  file: { name: string };
  suite?: { name: string; type?: string; suite?: unknown };
}

/** A test's id, as the flake reporter names it: `<file> > <suite> > … > <test>`. */
export function quarantineIdOf(task: Running): string {
  const names = [task.name];
  for (
    let at = task.suite as Running["suite"];
    at && at.type === "suite";
    at = at.suite as Running["suite"]
  ) {
    names.unshift(at.name);
  }
  return [task.file.name, ...names].join(" > ");
}

/** Whether this run skips `task`: it is in `ci/quarantine.toml` and this is not the nightly run. */
export function isQuarantined(
  task: Running,
  env: Record<string, string | undefined> = process.env,
): boolean {
  const skipped = skippedIds("vitest", env);
  return skipped.length > 0 && skipped.includes(quarantineIdOf(task));
}
