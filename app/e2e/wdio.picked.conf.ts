// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { appendFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { config as base } from "./wdio.conf.js";
import { anEmptyRecord, built, copyFixturePlane, theRunsEnvironment } from "./harness.js";
import { PANIC_LOG } from "./processes.js";

/**
 * The theme the fixture project picks in `[theme] use`, **before the app starts** (#2583): the
 * light one of Purlis Classic. The default theme is Classic's dark one, and it is drawn forced,
 * not following the system, so a window that ignored the pick is drawn dark on any runner.
 */
export const PICKED = "purlis-light";

// Through the environment, and made only if it is not already there: WebdriverIO's worker
// imports this file again, and the spec has to read the SAME plane the launcher wrote.
const plane = (process.env.PURLIS_PICKED_PLANE ??= (() => {
  const made = copyFixturePlane();
  // A project picks in its Shared file (`purlis_core::extension::project::theme`).
  appendFileSync(join(made, "charter.toml"), `\n[theme]\nuse = "${PICKED}"\n`);
  return made;
})());

/**
 * **A project whose theme is picked before launch** (#2583): the window is created on it, so the
 * first frame and the native window take the pick from the theme levels, as a person's launch
 * does. WebdriverIO's Tauri service keeps one app process for a whole run, and the base run's
 * project picks nothing, so this is a run of its own, as `wdio.layout.conf.ts` is: `wdio.conf.ts`
 * excludes the specs by name.
 */
export const config: WebdriverIO.Config = {
  ...base,
  specs: ["./specs/**/*.picked.e2e.ts"],
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
