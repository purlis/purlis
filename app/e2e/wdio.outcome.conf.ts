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
  writeShell,
} from "./harness.js";
import { harnessUnderTest } from "./outcomeBars.js";
import { PANIC_LOG } from "./processes.js";

/**
 * **The Skill studio's outcome bars, one run per harness** (SK-23, #4343), each on its stand-in:
 * `PURLIS_OUTCOME_HARNESS=claude|codex|opencode npm run e2e:outcome`.
 *
 * CI judges them on a `dev-release` build (#4351). To measure what CI measures, build with
 * `npx tauri build --no-bundle --features e2e --config src-tauri/tauri.e2e.conf.json -- -p
 * purlis-app -p fake-harness -p purlis-cli --profile dev-release` and run with
 * `CHARTER_TARGET_DIR=<the checkout>/target/dev-release`; without it the run takes the debug build.
 * The studio's opening is judged against its 300 ms bar, except with `CI=true`, where it is the
 * median guard (`openJudged`).
 *
 * A run of its own, since the app keeps one process and one harness for a run, and the bars are
 * about one harness at a time; and a plane of its own, since two hundred skills would reach every
 * other spec. Codex's profile is the state run's, the only way a Codex chat starts (#2037), with
 * the harness that behaves as Codex does; Claude Code's is a sandbox-only machine profile as well
 * on Linux, where every sandboxed Claude Code chat runs inside purlis's wrap (ADR 0067 as amended
 * 2026-11-10). opencode's reports through hooks and names its conversation
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
});
declareTheRunsProfile(plane, program, environment, kind);

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
        appBinaryPath: built(process.platform === "win32" ? "purlis-app.exe" : "purlis-app"),
        captureBackendLogs: true,
        captureFrontendLogs: true,
        env: environment,
      },
    ],
  ],
};
