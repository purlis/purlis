// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { join } from "node:path";
import process from "node:process";
import { describe, expect, it } from "vitest";
import {
  LONG_ASKED_COMMAND,
  singleQuoted,
  theRunsEnvironment,
  THE_RUNS_GIT_IDENTITY,
} from "./harness.js";

/**
 * A path a launcher writes into a script reaches `sh` as one literal word (SC-15). A run's tree
 * is under the system's temporary directory, so a path is not ours to promise anything about.
 */
describe("a word written into a launcher", () => {
  it.each([
    "/plain/path",
    "/with space/app",
    "/it's/quoted",
    "/$HOME/and/`date`/and/$(id)",
    '/a "double" quote and a \\ backslash',
  ])("reaches sh exactly as written: %s", (word) => {
    const said = execFileSync("sh", ["-c", `printf %s ${singleQuoted(word)}`], {
      encoding: "utf8",
    });
    expect(said).toBe(word);
  });
});

/**
 * Every app a run launches commits as the run, whatever the runner's git config says (#1250):
 * a Linux runner has no identity, and the doctor's finding then stood in every spec's band.
 */
describe("the home a launched app is given", () => {
  it("holds the run's own git identity, the one the doctor's cleared git reads", () => {
    const env = theRunsEnvironment("/unused");
    // As charter's hardened runner asks it: a cleared environment, `HOME` alone.
    const ask = (key: string) =>
      execFileSync("git", ["config", "--global", "--get", key], {
        encoding: "utf8",
        env: { HOME: env.HOME, PATH: process.env.PATH ?? "" },
      }).trim();
    expect(ask("user.name")).toBe(THE_RUNS_GIT_IDENTITY.name);
    expect(ask("user.email")).toBe(THE_RUNS_GIT_IDENTITY.email);
    expect(env.GIT_CONFIG_GLOBAL).toBe(join(env.HOME, ".gitconfig"));
    expect(env.HOME.startsWith(env.CHARTER_PLANE_FENCE)).toBe(true);
  });
});

/**
 * The long ask's line is exactly as long as an ask's summary may be (`SUMMARY_WIDTH`, 200 in
 * `crates/purlis-core/src/harness/model.rs`): one character more and the core cuts it, and the
 * Inbox spec would be reading an ellipsis rather than the whole line (#1146).
 */
describe("the long ask the reporting harness raises", () => {
  it("is a 200-character line, with the words the core puts before a command", () => {
    expect(`Run a command: ${LONG_ASKED_COMMAND}`).toHaveLength(200);
  });
});
