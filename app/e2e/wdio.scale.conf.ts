// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import process from "node:process";
import { config as base } from "./wdio.conf.js";
import { anEmptyRecord, built, copyFixturePlane, theRunsEnvironment } from "./harness.js";
import { PANIC_LOG } from "./processes.js";

/**
 * **The nightly scale run's window** (#5497, `.github/workflows/scale.yml`): the app opened on a
 * project at the scale spec's target size, for what only the real app can measure: its sidebar
 * answer, its watcher's setup and its idle CPU with a window shown
 * (`specs/*.scale.e2e.ts`). Evidence only, so it is a run of its own, and `wdio.conf.ts`
 * excludes its specs by name.
 *
 * The project is `scale-project generate`'s, made by the workflow inside the run's tree
 * (`$CHARTER_SCENARIO_RUN`) before this starts, and named by `$PURLIS_SCALE_PLANE`, so the fence
 * holds it as it holds every other run's plane. Without the variable the run opens a copy of the
 * daily fixture: a check of the run itself, at no scale.
 */
const plane = (process.env.PURLIS_SCALE_PLANE ??= copyFixturePlane());

export const config: WebdriverIO.Config = {
  ...base,
  specs: ["./specs/**/*.scale.e2e.ts"],
  // The base run EXCLUDES these specs; spreading its `exclude` would exclude them here too.
  exclude: [],
  // A minute of idle CPU, and a sidebar answered across a thousand workspaces.
  mochaOpts: { ui: "bdd", timeout: 900_000 },
  beforeSession() {
    anEmptyRecord(plane);
  },
  services: [
    [
      "@wdio/tauri-service",
      {
        appBinaryPath: built(process.platform === "win32" ? "charter-app.exe" : "charter-app"),
        captureBackendLogs: true,
        captureFrontendLogs: false,
        env: theRunsEnvironment(plane, { CHARTER_PANIC_LOG: PANIC_LOG }),
      },
    ],
  ],
};
