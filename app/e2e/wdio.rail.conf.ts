// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import process from "node:process";
import { config as base } from "./wdio.conf.js";
import { anEmptyRecord, built, copyFixturePlane, theRunsEnvironment } from "./harness.js";
import { PANIC_LOG } from "./processes.js";

// Through the environment, and made only if it is not already there: WebdriverIO's worker
// imports this file again, and the spec has to read the SAME plane the launcher wrote.
const plane = (process.env.PURLIS_RAIL_PLANE ??= copyFixturePlane());

/**
 * **The project rail's own run** (#3077). A show or pin the person presses holds over the
 * window's own hide for the rest of the launch (`app/src/railMode.ts`), and WebdriverIO's Tauri
 * service keeps one app process for a whole run. The base run's specs measure narrow windows with
 * the rail as the window draws it, so the spec that presses the rail's show and keep is a run of
 * its own, as `wdio.picked.conf.ts` is: `wdio.conf.ts` excludes it by name.
 */
export const config: WebdriverIO.Config = {
  ...base,
  specs: ["./specs/**/*.rail.e2e.ts"],
  // The base run EXCLUDES these specs; spreading its `exclude` would exclude them here too.
  exclude: [],
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
        env: theRunsEnvironment(plane, { CHARTER_PANIC_LOG: PANIC_LOG }),
      },
    ],
  ],
};
