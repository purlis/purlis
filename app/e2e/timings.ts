// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { logLine } from "./processes.js";

/**
 * How long each scenario test and each spec file's session took, one line of JSON each in
 * `logs/timings.jsonl`, for `tools/e2e-timings.mjs` to rank as notices on the `scenario` jobs.
 * The specs run one at a time in one app, so what makes a run slow is which specs are, and a
 * notice is the one place that can be read without the job's log.
 *
 * Hooks in the config rather than a reporter, for the reason `failures.ts` gives.
 */
export const TIMINGS = "timings.jsonl";

/** When this worker's session began: each spec file runs in a worker of its own. */
let sessionBegan: number | undefined;

/** Called from `beforeSession`. */
export function aSessionBegins(): void {
  sessionBegan = Date.now();
}

/** Called from `afterSession`: the spec file's whole time, its hooks and its app's start too. */
export function aSessionEnds(specs: string[]): void {
  if (sessionBegan === undefined) return;
  logLine(TIMINGS, { kind: "spec", file: specs[0], ms: Date.now() - sessionBegan });
}

/** What `afterTest` is handed about the test that just ran. */
interface Ran {
  title: string;
  parent?: string;
  file?: string;
}

/** Called from `afterTest`, passed or not, and from `afterHook` with `kind` "hook": a spec's
 *  `before` and `after` are where a spec that ends or opens many chats spends its time. */
export function recordTiming(ran: Ran, ms: number, kind: "test" | "hook" = "test"): void {
  logLine(TIMINGS, { kind, file: ran.file, parent: ran.parent, title: ran.title, ms });
}
