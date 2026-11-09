// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import {
  chmodSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import { describe, expect, it } from "vitest";
import {
  copyFixturePlane,
  LONG_ASKED_COMMAND,
  SCENARIO_CONVERSATION,
  singleQuoted,
  theRunsEnvironment,
  THE_RUNS_GIT_IDENTITY,
  writeReportingShell,
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

/**
 * A fixture copy says nothing about which workspace was worked in last (#2715). The first
 * open pins the most recently active workspaces, newest first, off these times; a copy that
 * kept the times it was written at would order the strip by the filesystem's listing.
 */
describe("a copy of a fixture plane", () => {
  it("gives every file and directory in it one modification time", () => {
    const root = copyFixturePlane();
    const times = new Set<number>();
    const walk = (dir: string): void => {
      times.add(statSync(dir).mtimeMs);
      for (const entry of readdirSync(dir, { withFileTypes: true })) {
        const path = join(dir, entry.name);
        if (entry.isDirectory()) walk(path);
        else if (entry.isFile()) times.add(statSync(path).mtimeMs);
      }
    };
    walk(join(root, "workspaces"));
    walk(join(root, ".charter"));
    expect(times.size).toBe(1);
  });
});

/**
 * **The reporting harness names its conversation on every hook it reports, where it names one**
 * (#5900). A Codex chat runs sandboxed at the floor, where a hook line the app does not take in
 * time is lost, not spooled: a stand-in that named its conversation only in its first hook had
 * nothing left to restart on when the app was slow to start and missed that one line.
 *
 * Run with a `charter` that writes down each hook and what it was handed, and a harness that runs
 * each `--hook` it is given, so the shell's own hooks are what is read.
 */
describe("the hooks the reporting harness reports", () => {
  function hooksOf(namesItsConversation: boolean): { hook: string; told: string }[] {
    const scratch = mkdtempSync(join(tmpdir(), "reporting-shell-"));
    const heard = join(scratch, "heard");
    const charter = join(scratch, "charter");
    writeFileSync(
      charter,
      `#!/bin/sh\n[ "$1" = hook ] && printf '%s\\t%s\\n' "$2" "$(cat)" >> ${singleQuoted(heard)}\nexit 0\n`,
    );
    const harness = join(scratch, "fake-harness");
    writeFileSync(
      harness,
      '#!/bin/sh\nwhile [ $# -gt 0 ]; do\n  [ "$1" = --hook ] && { sh -c "$2"; shift; }\n  shift\ndone\n',
    );
    chmodSync(charter, 0o755);
    chmodSync(harness, 0o755);
    execFileSync(writeReportingShell(harness, charter, namesItsConversation), { stdio: "ignore" });
    return readFileSync(heard, "utf8")
      .split("\n")
      .filter((line) => line !== "")
      .map((line) => {
        const [hook, told] = line.split("\t");
        return { hook, told };
      });
  }

  it("names the conversation on each of them where it names one", () => {
    const named = hooksOf(true).map(({ hook, told }) => ({
      hook,
      conversation: told === "" ? undefined : JSON.parse(told).session_id,
    }));
    expect(named).toEqual(
      ["sessionstart", "userpromptsubmit", "stop"].map((hook) => ({
        hook,
        conversation: SCENARIO_CONVERSATION,
      })),
    );
  });

  it("hands them nothing where it names none", () => {
    expect(hooksOf(false)).toEqual([
      { hook: "userpromptsubmit", told: "" },
      { hook: "stop", told: "" },
    ]);
  });
});
