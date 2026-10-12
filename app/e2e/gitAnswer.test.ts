// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { aGitAnswerIsWaiting, noGitAnswerWaits, theNextLookForGitFindsNone } from "./gitAnswer.js";
import {
  GIT_ANSWER_VARIABLE,
  THE_RUNS_GIT_ANSWER,
  THE_RUNS_TREE,
  theRunsEnvironment,
} from "./harness.js";

/**
 * The scenario's half of the git seam (#1817), checked here because the spec that uses it first
 * runs in CI: a spelling the app does not read would leave the spec waiting on a guided screen
 * that never comes.
 */

const here = dirname(fileURLToPath(import.meta.url));
const firstrun = () => readFileSync(join(here, "..", "src-tauri", "src", "firstrun.rs"), "utf8");

afterEach(noGitAnswerWaits);

describe("a look for git that finds none", () => {
  it("is named by the variable the e2e build reads, in every launcher's environment", () => {
    expect(firstrun()).toContain(`const GIT_ANSWER: &str = "${GIT_ANSWER_VARIABLE}";`);
    expect(theRunsEnvironment(join(THE_RUNS_TREE, "plane"))[GIT_ANSWER_VARIABLE]).toBe(
      THE_RUNS_GIT_ANSWER,
    );
  });

  it("waits until the window looks, and is taken back when nothing looked", () => {
    theNextLookForGitFindsNone();
    expect(aGitAnswerIsWaiting()).toBe(true);
    noGitAnswerWaits();
    expect(aGitAnswerIsWaiting()).toBe(false);
  });
});
