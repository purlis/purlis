// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import process from "node:process";
import { config as base } from "./wdio.conf.js";
import {
  anEmptyRecord,
  built,
  copyFixturePlane,
  declareAProfile,
  theRunsEnvironment,
} from "./harness.js";
import { writeBenchShell } from "./load.js";
import { PANIC_LOG } from "./processes.js";

/**
 * **The nightly scale run's window** (#5497, `.github/workflows/scale.yml`), what only the real
 * app can measure. Evidence only, so it is a run of its own, and `wdio.conf.ts` excludes its
 * specs by name. `$PURLIS_SCALE_RUN` says which of its two runs this is:
 *
 * - **`project`** (the default): the app opened on a project at the scale spec's target size,
 *   for its sidebar answer, its watcher's setup and its idle CPU with a window shown
 *   (`specs/scale.scale.e2e.ts`). The project is `scale-project generate`'s, made by the workflow
 *   inside the run's tree (`$CHARTER_SCENARIO_RUN`) before this starts and named by
 *   `$PURLIS_SCALE_PLANE`, so the fence holds it as it holds every other run's plane. Without the
 *   variable the run opens a copy of the daily fixture: a check of the run itself, at no scale.
 * - **`chat-events`**: busy chats firing hooks, on a copy of the daily fixture, for how many chat
 *   events reach the window a second (#4883, `specs/chat-events.scale.e2e.ts`). What it counts
 *   depends on how many chats are busy, not on how big the project is. The workflow runs it with
 *   `$PURLIS_E2E_CHAT_EVENTS_MS` unset and set to `0`, which the `e2e` build alone reads.
 */
const chatEvents = process.env.PURLIS_SCALE_RUN === "chat-events";

// Through the environment, and made only if it is not already there: WebdriverIO's worker
// imports this file again, and the spec has to read the SAME plane the launcher wrote.
const plane = chatEvents
  ? (process.env.PURLIS_SCALE_FIXTURE ??= copyFixturePlane())
  : (process.env.PURLIS_SCALE_PLANE ||= copyFixturePlane());

const knob = process.env.PURLIS_E2E_CHAT_EVENTS_MS;

// A chat's program is the fake harness, with the load the spec chose: the project's default
// profile runs it, as `wdio.conf.ts`'s runs its harness, and a new tab's `$SHELL` is it too.
const loaded = chatEvents ? writeBenchShell(built("fake-harness")) : undefined;
if (loaded) declareAProfile(plane, loaded);

export const config: WebdriverIO.Config = {
  ...base,
  specs: [chatEvents ? "./specs/chat-events.scale.e2e.ts" : "./specs/scale.scale.e2e.ts"],
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
        appBinaryPath: built(process.platform === "win32" ? "purlis-app.exe" : "purlis-app"),
        captureBackendLogs: true,
        captureFrontendLogs: false,
        env: theRunsEnvironment(plane, {
          CHARTER_PANIC_LOG: PANIC_LOG,
          // A new tab's pane runs `$SHELL`: the fake harness, with the load the spec chose.
          ...(loaded ? { SHELL: loaded } : {}),
          ...(knob === undefined ? {} : { PURLIS_E2E_CHAT_EVENTS_MS: knob }),
        }),
      },
    ],
  ],
};
