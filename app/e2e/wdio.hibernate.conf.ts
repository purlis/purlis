// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { config as base } from "./wdio.conf.js";
import {
  anEmptyRecord,
  built,
  copyFixturePlane,
  declareTheRunsProfile,
  theRunsEnvironment,
  writeReportingShell,
} from "./harness.js";
import { PANIC_LOG } from "./processes.js";

/**
 * **Hibernate a chat from its row, and Resume it** (SC-4a, #7630): a run of its own, because the
 * chat has to be on a harness that resumes by id and names its conversation (opencode's
 * stand-in), and the app reads its harness from `SHELL` once, at the start. The state run's
 * harness is Codex's, which names none, so a chat there is never hibernated.
 */
const plane = copyFixturePlane();
const harness = writeReportingShell(built("fake-harness"), built("charter"), true);
const environment = theRunsEnvironment(plane, {
  SHELL: harness,
  CHARTER_PANIC_LOG: PANIC_LOG,
});
declareTheRunsProfile(plane, harness, environment, "opencode");

export const config: WebdriverIO.Config = {
  ...base,
  specs: ["./specs/**/*.hibernate.e2e.ts"],
  // The base run excludes these; spreading it would exclude them here too.
  exclude: [],
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
