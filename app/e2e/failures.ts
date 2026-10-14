// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { logLine } from "./processes.js";

/**
 * Every failed scenario test and hook, one line of JSON each in `logs/failures.jsonl`, for
 * `tools/e2e-annotations.mjs` to turn into GitHub annotations when the `scenario` job fails.
 * An annotation is the one place a failure can be read without the job's log or its artifact.
 *
 * A hook in the config rather than a reporter: a reporter would import `@wdio/reporter`, which
 * the app has only as another package's dependency.
 */
export const FAILURES = "failures.jsonl";

/** What `afterTest` and `afterHook` are handed about the test or hook that just ran. */
interface Ran {
  title: string;
  parent?: string;
  file?: string;
}

/**
 * What else a failure carries for its annotation (#1949): the commands the window had not had
 * answered in time (`src/answerWatch.ts`), and the core's last log lines (`coreLogTail`).
 */
export interface Evidence {
  late?: readonly { command: string; ms: number | undefined }[];
  coreLog?: readonly string[];
  /** What lay over an element the test could not press or see (`covers.ts`). */
  covers?: readonly string[];
}

/** How many of the core's last log lines a failure carries. */
export const CORE_LOG_LINES = 40;

/**
 * The last `count` lines of the newest of the app's daily logs in `dir` (`charter.<date>.log` or
 * `purlis.<date>.log`, `purlis_core::applog`), which the app writes with anything secret-shaped
 * already replaced. Nothing where there is no log.
 */
export function coreLogTail(dir: string, count = CORE_LOG_LINES): string[] {
  if (!existsSync(dir)) return [];
  const logs = readdirSync(dir)
    .filter((name) => /^(charter|purlis)\..+\.log$/.test(name))
    .map((name) => join(dir, name))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  if (logs.length === 0) return [];
  const lines = readFileSync(logs[0], "utf8").split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines.slice(-count);
}

/** Writes `ran` down if it failed; a passing test or hook writes nothing. */
export function recordFailure(ran: Ran, error: unknown, evidence: Evidence = {}): void {
  if (!error) return;
  // An `Error`, or an object shaped like one once it has crossed a worker boundary.
  const shaped = typeof error === "object" ? (error as { message?: unknown; stack?: unknown }) : {};
  const message = typeof shaped.message === "string" ? shaped.message : String(error);
  const stack = typeof shaped.stack === "string" ? shaped.stack : undefined;
  logLine(FAILURES, {
    file: ran.file,
    parent: ran.parent,
    title: ran.title,
    message,
    stack,
    late: evidence.late,
    coreLog: evidence.coreLog,
    covers: evidence.covers,
  });
}
