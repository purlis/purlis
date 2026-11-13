// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { delimiter, join } from "node:path";
import process from "node:process";
import { config as base } from "./wdio.conf.js";
import {
  CLAUDE_CODE_VERSION,
  THE_RUNS_GIT_IDENTITY,
  anEmptyRecord,
  built,
  copyFixturePlane,
  theRunsEnvironment,
} from "./harness.js";
import { PANIC_LOG } from "./processes.js";

/** The variable naming the runner image the pool is made of, by digest (`.github/workflows/ci.yml`). */
export const POOL_IMAGE_VARIABLE = "PURLIS_E2E_POOL_IMAGE";

const image = process.env[POOL_IMAGE_VARIABLE];
if (!image)
  throw new Error(
    `${POOL_IMAGE_VARIABLE} names no runner image: the scenario job's pool leg builds one (e2e/pool-runner.Dockerfile)`,
  );

// Through the environment, and made only if it is not already there: WebdriverIO's worker
// imports this file again, and the app it starts has to open the SAME plane the launcher made a
// repository of.
const fresh = process.env.PURLIS_POOL_PLANE === undefined;
const plane = (process.env.PURLIS_POOL_PLANE ??= copyFixturePlane());

/**
 * **Pool setup's Docker path, in the real window** (#4985, #4484's first acceptance criterion),
 * and its cluster path on the job's kind cluster (#4494): a spec opens Pool setup and presses each
 * step's main button to Try it's last screen, on this machine's Docker or that cluster, with a
 * pool of the runner image this build makes. Its own run, on Linux alone, because it needs a
 * Docker daemon, that image in a registry of the job's own, and a profile whose harness reports
 * its start on the runner, none of which another run has.
 *
 * **What stands in for an account.** The profile is a Claude Code one, kept in this run's own
 * machine store, pinned to the version the runner image keeps installed. There, that `claude`
 * (`e2e/pool-runner.Dockerfile`, never on the image's `PATH`) says it is
 * signed in when asked, as Claude Code's own does, and otherwise reports its session's start
 * through the runner's `purlis hook` as Claude Code's `SessionStart` hook does, and waits. No
 * model is called. On this machine, the profile's `claude` answers its version and never reports
 * a start: a try whose chat ran here instead of on the pool never passes.
 */
const environment = theRunsEnvironment(plane, {
  CHARTER_PANIC_LOG: PANIC_LOG,
  PURLIS_NO_BACKGROUND_CHECKS: "1",
  // The agent the job's ssh signs in to the other machine with (#6266, tests/remote-docker/up.sh),
  // as a person's window has their own.
  ...(process.env.SSH_AUTH_SOCK ? { SSH_AUTH_SOCK: process.env.SSH_AUTH_SOCK } : {}),
});
// The kubeconfig the job's kind cluster was made in (#4494), where a person's own is: kubectl's
// default, in the home the app is given. The window's kubectl is never handed `KUBECONFIG`
// (`purlis_core::clusterpool::CLEARED`), so it reads this one on every call, and Pool setup keeps
// none of it.
if (process.env.KUBECONFIG) {
  const kube = join(environment.HOME, ".kube");
  mkdirSync(kube, { recursive: true, mode: 0o700 });
  copyFileSync(process.env.KUBECONFIG, join(kube, "config"));
  chmodSync(join(kube, "config"), 0o600);
}
const programs = declarePoolsClaudeProfile(
  plane,
  environment.CHARTER_CONFIG_HOME,
  environment.CHARTER_DATA_HOME,
);
environment.PATH = [programs, process.env.PATH ?? ""].join(delimiter);
// A project a pool's runner holds is sent its code as git sends it: a repository with a commit.
if (fresh) aRepository(plane);

export const config: WebdriverIO.Config = {
  ...base,
  specs: ["./specs/**/*.pool.e2e.ts"],
  // The base run EXCLUDES these specs; spreading its `exclude` would exclude them here too.
  exclude: [],
  // A runner made, its image pulled from the job's registry, a profile kept there and a chat
  // started on it: past the base run's three minutes.
  mochaOpts: { ...base.mochaOpts, timeout: 900_000 },
  beforeSession() {
    anEmptyRecord(plane);
  },
  services: [
    [
      "@wdio/tauri-service",
      {
        appBinaryPath: built(process.platform === "win32" ? "purlis-app.exe" : "purlis-app"),
        captureBackendLogs: true,
        captureFrontendLogs: true,
        env: environment,
      },
    ],
  ],
};

/**
 * A Claude Code profile `scenario` of this run's machine store, the plane's default, and the
 * folder of programs that holds its `claude` on this machine, to go first on the app's `PATH`.
 */
function declarePoolsClaudeProfile(plane: string, configHome: string, dataHome: string): string {
  const bin = join(`${plane}.programs`, "bin");
  mkdirSync(bin, { recursive: true });
  const claude = join(bin, "claude");
  writeFileSync(
    claude,
    [
      "#!/bin/sh",
      "# Written by the scenario tests: Claude Code on this machine, for Pool setup's run. It",
      "# reports no start, so only a chat on the pool's runner passes Try it.",
      'case "$1" in auth) exit 0 ;; esac',
      `for word in "$@"; do [ "$word" = --version ] && { echo '${CLAUDE_CODE_VERSION}'; exit 0; }; done`,
      "exec sleep 3600",
      "",
    ].join("\n"),
  );
  chmodSync(claude, 0o755);
  const folder = join(dataHome, "profiles", "scenario", "config");
  mkdirSync(folder, { recursive: true, mode: 0o700 });
  const store = join(configHome, "charter");
  mkdirSync(store, { recursive: true, mode: 0o700 });
  writeFileSync(
    join(store, "profiles.json"),
    JSON.stringify({
      version: 1,
      profiles: [
        {
          name: "scenario",
          harness: "claude",
          title: "scenario",
          folders: { config: folder },
          // Pinned to the version the runner image keeps installed (`e2e/pool-runner.Dockerfile`),
          // so the runner runs it and fetches nothing from Claude Code's release.
          pinned: { version: CLAUDE_CODE_VERSION.split(" ")[0], at: Math.floor(Date.now() / 1000) },
          // Try it readies its chat on this desktop first, and on a Linux one a sandboxed Claude
          // Code chat starts only on a sandbox-only profile (ADR 0067 as amended 2026-11-10).
          sandbox_only: process.platform === "linux",
        },
      ],
    }) + "\n",
    { mode: 0o600 },
  );
  writeFileSync(join(plane, "charter.local.toml"), '[harness]\ndefault = "scenario"\n');
  return bin;
}

/** Makes the plane at `root` a repository with one commit. */
function aRepository(root: string): void {
  const git = (args: string[]) =>
    execFileSync(
      "git",
      [
        "-c",
        `user.name=${THE_RUNS_GIT_IDENTITY.name}`,
        "-c",
        `user.email=${THE_RUNS_GIT_IDENTITY.email}`,
        ...args,
      ],
      { cwd: root, stdio: "ignore" },
    );
  git(["init", "-q", "-b", "main"]);
  git(["add", "-A"]);
  git(["commit", "-q", "-m", "a project"]);
}
