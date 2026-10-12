// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, rmSync, writeFileSync } from "node:fs";
import { THE_RUNS_GIT_ANSWER } from "./harness.js";

/**
 * **A machine with no git, for one look** (#1817, #1813's guided screen): the seam a scenario
 * spec uses in place of a runner it cannot take git away from.
 *
 * The app built with the `e2e` feature answers the window's next look for git (`git_presence`)
 * with "missing" while this file is there, and removes it as it reads it (`scenario_git` in
 * `src-tauri/src/firstrun.rs`), so Check again asks the machine. The runner's own gate is never
 * stood in for: git that the app runs is still checked for on the machine. Every other build
 * reads nothing.
 */

/** Has the window's next look for git find none. */
export function theNextLookForGitFindsNone(): void {
  writeFileSync(THE_RUNS_GIT_ANSWER, "missing");
}

/** Whether that answer is still waiting: the window has not looked for git since. */
export function aGitAnswerIsWaiting(): boolean {
  return existsSync(THE_RUNS_GIT_ANSWER);
}

/** Takes back an answer nothing used, so it cannot answer another spec's look. */
export function noGitAnswerWaits(): void {
  rmSync(THE_RUNS_GIT_ANSWER, { force: true });
}
