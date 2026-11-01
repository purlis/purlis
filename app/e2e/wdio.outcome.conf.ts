// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { config as base } from "./wdio.conf.js";
import {
  anEmptyRecord,
  built,
  copyFixturePlane,
  declareAProfile,
  declareTheStateRunsProfile,
  theRunsEnvironment,
  writeReportingShell,
  writeShell,
} from "./harness.js";
import { harnessUnderTest } from "./outcomeBars.js";
import { PANIC_LOG } from "./processes.js";

/**
 * **The Skill studio's outcome bars, one run per harness** (SK-23, #4343), each on its stand-in:
 * `PURLIS_OUTCOME_HARNESS=claude|codex|opencode npm run e2e:outcome`.
 *
 * A run of its own, since the app keeps one process and one harness for a run, and the bars are
 * about one harness at a time; and a plane of its own, since two hundred skills would reach every
 * other spec. Codex's profile is the state run's, the only way a Codex chat starts (#2037), with
 * the harness that behaves as Codex does. opencode's reports through hooks and names its conversation
 * the same way, so a restart has something to resume.
 */
const { kind } = harnessUnderTest(process.env.PURLIS_OUTCOME_HARNESS);

const plane = copyFixturePlane();
const program =
  kind === "claude"
    ? writeShell(built("fake-harness"))
    : writeReportingShell(built("fake-harness"), built("charter"), true);
const environment = theRunsEnvironment(plane, {
  SHELL: program,
  CHARTER_PANIC_LOG: PANIC_LOG,
  PURLIS_NO_BACKGROUND_CHECKS: "1",
  PURLIS_SKILL_STUDIO: "1",
});
if (kind === "codex") declareTheStateRunsProfile(plane, program, environment);
else declareAProfile(plane, program, kind);

export const config: WebdriverIO.Config = {
  ...base,
  specs: ["./specs/**/*.outcome.e2e.ts"],
  // The base run excludes these, and one that finds no specs does not say so loudly.
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
        env: environment,
      },
    ],
  ],
};
