// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import { describe, expect, it } from "vitest";
import {
  CLAUDE_CODE_VERSION,
  copyFixturePlane,
  declareTheRunsProfile,
  LONG_ASKED_COMMAND,
  SCENARIO_CONVERSATION,
  singleQuoted,
  theRunsEnvironment,
  theRunsStandIns,
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
    try {
      execFileSync(writeReportingShell(harness, charter, namesItsConversation), {
        stdio: "ignore",
      });
      return readFileSync(heard, "utf8")
        .split("\n")
        .filter((line) => line !== "")
        .map((line) => {
          const [hook, told] = line.split("\t");
          return { hook, told };
        });
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
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

/**
 * **The run's profile, where a sandboxed chat runs only on a sandbox-only one** (#2037; ADR 0067
 * as amended 2026-11-10). On Linux every sandboxed Claude Code chat runs inside purlis's wrap, on
 * a sandbox-only profile, so the run's `scenario` is one of its machine store, as Codex's already
 * was, and a profile the project declared under that name would take its place.
 */
describe("the run's profile", () => {
  /** Runs `body` as if on `platform`, with the run's stand-ins in a folder of its own. */
  function on<T>(platform: NodeJS.Platform, body: (scratch: string) => T): T {
    const was = Object.getOwnPropertyDescriptor(process, "platform");
    const target = process.env.CHARTER_TARGET_DIR;
    const scratch = mkdtempSync(join(tmpdir(), "the-runs-profile-"));
    Object.defineProperty(process, "platform", { value: platform });
    process.env.CHARTER_TARGET_DIR = join(scratch, "target");
    try {
      return body(scratch);
    } finally {
      if (was) Object.defineProperty(process, "platform", was);
      if (target === undefined) delete process.env.CHARTER_TARGET_DIR;
      else process.env.CHARTER_TARGET_DIR = target;
      rmSync(scratch, { recursive: true, force: true });
    }
  }

  /** A program that says `said` and nothing else, as the run's harness stands in. */
  function saying(scratch: string, name: string, said: string): string {
    const program = join(scratch, name);
    writeFileSync(program, `#!/bin/sh\necho ${singleQuoted(said)}\n`);
    chmodSync(program, 0o755);
    return program;
  }

  /** The run's profile declared for `kind`: the plane, its environment and its local file. */
  function declared(scratch: string, kind: string) {
    const plane = join(scratch, "plane");
    mkdirSync(plane);
    const environment: Record<string, string> = {
      CHARTER_CONFIG_HOME: join(scratch, "config"),
      CHARTER_DATA_HOME: join(scratch, "data"),
      PATH: "/usr/bin:/bin",
    };
    declareTheRunsProfile(
      plane,
      saying(scratch, "harness", "the run's harness"),
      environment,
      kind,
    );
    const store = join(environment.CHARTER_CONFIG_HOME, "charter", "profiles.json");
    return {
      plane,
      environment,
      local: readFileSync(join(plane, "charter.local.toml"), "utf8"),
      store: existsSync(store) ? (JSON.parse(readFileSync(store, "utf8")) as unknown) : undefined,
    };
  }

  /** What `program` on `environment`'s `PATH` answers `args` with. */
  function answer(environment: Record<string, string>, program: string, args: string[]) {
    return execFileSync(program, args, {
      encoding: "utf8",
      env: { PATH: environment.PATH },
    }).trim();
  }

  it("is a sandbox-only Claude Code profile of the machine store on Linux, the project's default", () => {
    on("linux", (scratch) => {
      const { environment, local, store } = declared(scratch, "claude");
      const folder = join(environment.CHARTER_DATA_HOME, "profiles", "scenario", "config");
      expect(store).toEqual({
        version: 1,
        profiles: [
          {
            name: "scenario",
            harness: "claude",
            title: "scenario",
            folders: { config: folder },
            sandbox_only: true,
          },
        ],
      });
      expect(statSync(folder).isDirectory()).toBe(true);
      // The project makes it the default, and declares no profile of that name to take its place.
      expect(local).toContain('default = "scenario"');
      expect(local).not.toContain("[harness.scenario]");
      expect(local).toContain("[harness.needs-approval]");
      // Its `claude` is first on the app's `PATH`: signed in, Claude Code by its version, and the
      // run's harness for everything else.
      expect(environment.PATH.endsWith(":/usr/bin:/bin")).toBe(true);
      execFileSync("claude", ["auth", "status"], { env: { PATH: environment.PATH } });
      expect(answer(environment, "claude", ["--version"])).toBe(CLAUDE_CODE_VERSION);
      expect(answer(environment, "claude", ["--session-id", "x"])).toBe("the run's harness");
    });
  });

  it("writes down what a chat on it saw in its config folder, where the plane's names lead", () => {
    on("linux", (scratch) => {
      const { plane, environment } = declared(scratch, "claude");
      const folder = join(environment.CHARTER_DATA_HOME, "profiles", "scenario", "config");
      // Made by purlis before the wrap; the one place there this writes.
      mkdirSync(join(folder, "debug"));
      const said = execFileSync("claude", ["--session-id", "the-session"], {
        encoding: "utf8",
        env: {
          PATH: environment.PATH,
          CHARTER_ROOT: plane,
          CHARTER_CHAT: "7",
          CHARTER_FOOTER: "kept",
          CLAUDE_CONFIG_DIR: folder,
        },
      }).trim();
      expect(said).toBe("the run's harness");
      const read = (name: string) => readFileSync(join(plane, ".charter", name), "utf8");
      expect(read("scenario/footer-7")).toBe("kept");
      expect(read("scenario-harness/session-7")).toBe("the-session");
      expect(readFileSync(join(folder, "debug", "scenario-harness", "session-7"), "utf8")).toBe(
        "the-session",
      );
    });
  });

  it("runs the run's stand-in Claude Code instead while a spec has put one there", () => {
    on("linux", (scratch) => {
      const { environment } = declared(scratch, "claude");
      const standIns = theRunsStandIns();
      saying(standIns, "claude", "the stand-in");
      expect(answer(environment, "claude", ["--session-id", "x"])).toBe("the stand-in");
      rmSync(join(standIns, "claude"));
      expect(answer(environment, "claude", ["--session-id", "x"])).toBe("the run's harness");
    });
  });

  it("is the project's own Claude Code profile on macOS, with nothing in the machine store", () => {
    on("darwin", (scratch) => {
      const { environment, local, store } = declared(scratch, "claude");
      expect(store).toBeUndefined();
      expect(local).toContain("[harness.scenario]");
      expect(local).toContain("[harness.needs-approval]");
      expect(environment.PATH).toBe("/usr/bin:/bin");
    });
  });

  it("is Codex's sandbox-only profile, as the state run had it, on Linux and macOS", () => {
    for (const platform of ["linux", "darwin"] as const)
      on(platform, (scratch) => {
        const { environment, local, store } = declared(scratch, "codex");
        expect(local).toBe('[harness]\ndefault = "scenario"\n');
        expect(store).toMatchObject({ profiles: [{ name: "scenario", harness: "codex" }] });
        execFileSync("codex", ["login", "status"], { env: { PATH: environment.PATH } });
        expect(answer(environment, "codex", ["exec"])).toBe("the run's harness");
      });
  });
});
