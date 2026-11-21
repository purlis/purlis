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
  const skipped = skippedFor(env);
  return skipped.length > 0 && skipped.includes(quarantineIdOf(task));
}

/** The ids this run skips, read once per file and event: a global `beforeEach` asks for every test. */
const read = new Map<string, string[]>();
function skippedFor(env: Record<string, string | undefined>): string[] {
  const key = [env.CI, env.GITHUB_EVENT_NAME, env.PURLIS_QUARANTINE_FILE].join("\0");
  let ids = read.get(key);
  if (ids === undefined) {
    ids = skippedIds("vitest", env);
    read.set(key, ids);
  }
  return ids;
}
