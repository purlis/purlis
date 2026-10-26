// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { config as base } from "./wdio.conf.js";
import {
  THE_RUNS_GIT_CONFIG_TEXT,
  THE_RUNS_TREE,
  anEmptyRecord,
  built,
  copyFixturePlane,
  theRunsEnvironment,
} from "./harness.js";
import { PANIC_LOG } from "./processes.js";

// Through the environment, and made only if it is not already there: WebdriverIO's worker
// imports this file again, and the spec has to read the SAME plane the launcher wrote.
const plane = (process.env.PURLIS_BROWSER_PLANE ??= copyFixturePlane());

/**
 * **The browser's own run** (#2025, spec #2008's one window end-to-end test): a browser view tab
 * draws frames from a real engine and takes a click. Its own run, because its session host
 * downloads the pinned engine on its first browser, which no other spec should wait on.
 *
 * **On Linux the engine is where a person's would be.** Its data home is `~/.local/share/purlis`
 * of a `$HOME` one folder below the temporary folder: the AppArmor profile the `.deb` installs
 * names the engine there, and CI tells AppArmor that folder holds homes, as the `rust` job does
 * for the window's end-to-end browser tests (#2835). On macOS the run's own homes serve.
 */
const linux = process.platform === "linux";
const home = THE_RUNS_TREE;
if (linux) writeFileSync(join(home, ".gitconfig"), THE_RUNS_GIT_CONFIG_TEXT);
/** On Linux, the home and data home the engine's AppArmor profile names; elsewhere the run's. */
const whereTheEngineIs: Record<string, string> = linux
  ? { HOME: home, CHARTER_DATA_HOME: join(home, ".local", "share", "purlis") }
  : {};

export const config: WebdriverIO.Config = {
  ...base,
  specs: ["./specs/**/*.browser.e2e.ts"],
  // The base run EXCLUDES these specs; spreading its `exclude` would exclude them here too.
  exclude: [],
  // The engine's download first: past the base run's three minutes.
  mochaOpts: { ...base.mochaOpts, timeout: 600_000 },
  beforeSession() {
    anEmptyRecord(plane);
  },
  services: [
    [
      "@wdio/tauri-service",
      {
        appBinaryPath: built(process.platform === "win32" ? "charter-app.exe" : "charter-app"),
        captureBackendLogs: true,
        captureFrontendLogs: true,
        env: theRunsEnvironment(plane, {
          CHARTER_PANIC_LOG: PANIC_LOG,
          ...whereTheEngineIs,
          PURLIS_NO_BACKGROUND_CHECKS: "1",
        }),
      },
    ],
  ],
};
