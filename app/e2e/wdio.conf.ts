// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readdirSync } from "node:fs";
import { delimiter, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import process from "node:process";
import {
  anEmptyRecord,
  built,
  cloneTheFixtureRepos,
  copyFixturePlane,
  cutAFixturePiece,
  declareTheRunsProfile,
  launchedUnderLaunchdsLimit,
  theRunsEnvironment,
  theRunsStandIns,
  writeForgeCache,
  writeShell,
} from "./harness.js";
import { browser } from "@wdio/globals";
import {
  coveredIn,
  describeEveryMissedPress,
  forgetTheMiss,
  messageOf,
  theMissBehind,
  whatWasOver,
} from "./covers.js";
import { theWindowIsDrawn } from "./drawn.js";
import { theProjectsFoldersAreAccepted } from "./acting.js";
import { noDialogIsLeftUp } from "./opening.js";
import { theWindowHoldsTheRail } from "./rail.js";
import { LOGS, PANIC_LOG, collectEvidence } from "./processes.js";
import { type Evidence, coreLogTail, recordFailure } from "./failures.js";
import { whatTheWindowShows } from "./windowShows.js";
import { aSessionBegins, aSessionEnds, recordTiming } from "./timings.js";
import { specsOfPart } from "./parts.js";

/**
 * The scenario tests: WebdriverIO driving the real app, with the fake harness standing in for
 * Claude Code or Codex.
 *
 * A build made with `src-tauri/tauri.e2e.conf.json` has its OWN app identifier, so the
 * single-instance plugin does not confuse it with a charter the operator happens to be
 * running. Sharing one made a dev app silently break every scenario run: the app under test
 * handed itself off to the other one and exited 0, and the service reported only "the app
 * likely crashed during startup".
 *
 * The app is driven through the WebDriver server inside it (`driverProvider: "embedded"`,
 * the service's default), which is the only path that works on macOS as well as Linux — see
 * the service's ADR 0002. Both plugins are behind the app's `e2e` cargo feature.
 */
const app = built(process.platform === "win32" ? "purlis-app.exe" : "purlis-app");

/**
 * What the service starts. A stress run starts the app the way launchd would, with a soft
 * open-file limit of 256, so that its two hundred chats show the app raised it (SC-15). Every
 * other run starts the binary itself.
 */
const launched = process.env.STRESS === "1" ? launchedUnderLaunchdsLimit(app) : app;

// The plane the app is started in: a fresh copy of a fixture plane, never the committed one.
const plane = copyFixturePlane();
// Every session the app opens is the fake harness, because that is the shell it finds.
const shell = writeShell(built("fake-harness"));
/**
 * What the app is started with. The plane, this run's own machine store and the fence come from
 * one place (`theRunsEnvironment`), because a launcher that pins two of the three reads exactly
 * like one that pins all three — and that is how charter-app#129 stayed invisible.
 */
const environment = theRunsEnvironment(plane, {
  SHELL: shell,
  // A panic that ends the app is written where a failed run keeps it (charter-app#16).
  CHARTER_PANIC_LOG: PANIC_LOG,
  // A spec's stand-in for an installed harness is found first, outside every folder a
  // sandboxed chat may write (`theRunsStandIns`).
  PATH: [theRunsStandIns(), process.env.PATH ?? ""].join(delimiter),
  // No forge refresh in a scenario run: the forge cache is the one `writeForgeCache`
  // left, and the Changes view's specs read it. A refresh starts on the first focus
  // that draws the Changes view, runs the runner's own `gh` against the network, and
  // overwrote that cache with "no pipeline recorded" whenever a spec before
  // `regions.e2e.ts` drew the view (train 76's `no-blank-view`). What a refresh
  // writes is `glrefresh`'s tests'.
  PURLIS_NO_BACKGROUND_CHECKS: "1",
  // The Skill studio's slices are built behind its flag (#2786), and a run checks them
  // as the dev channel's people see them: on (`specs/skill-studio.e2e.ts`).
  PURLIS_SKILL_STUDIO: "1",
});
// The plane's harness profiles, which are what the picker picks from. A declared one is
// deliberately NOT approved: approving it is the operator's click, and the scenario makes it. On
// Linux, where every sandboxed Claude Code chat runs inside purlis's wrap, on a sandbox-only
// profile (ADR 0067 as amended 2026-11-10), `scenario` is the run's machine profile, and its
// `claude` goes first on the app's `PATH` (`declareTheRunsProfile`).
declareTheRunsProfile(plane, shell, environment);
// The repos panel is about what git says, and the committed fixture cannot carry a `.git`
// directory — so the copy this run works on gets real clones, and the forge cache a
// refresher would have left. Neither is the app's doing: the app only reads them.
cloneTheFixtureRepos(plane);
// And one real piece cut off `svc`, because the explorer's whole content is the pieces a
// workspace holds and a plane with none of them cannot tell an empty listing from a drawn one.
cutAFixturePiece(plane);
writeForgeCache(plane);

/**
 * What a failure carries for its annotation (#1949): the commands the window had not had answered
 * in time, asked of the window itself, and the core's last log lines. A window that does not
 * answer leaves the first out, not the failure.
 */
async function evidence(error?: unknown): Promise<Evidence> {
  const late = await browser
    .execute(
      () =>
        (
          globalThis as { __purlisLateAnswers?: () => { command: string; ms?: number }[] }
        ).__purlisLateAnswers?.() ?? [],
    )
    .catch(() => []);
  // And, where the test could not press or see an element, what lay over it: as the press
  // missed, where `describeEveryMissedPress` took it then (#5609), or else asked now.
  const message = messageOf(error);
  const covered = coveredIn(message);
  const missed = theMissBehind(message);
  return {
    late: late.map((one) => ({ command: one.command, ms: one.ms })),
    coreLog: coreLogTail(LOGS),
    covers: missed ?? (covered === undefined ? undefined : await whatWasOver(covered)),
    shown: await whatTheWindowShows(),
  };
}

const STRESS_SPEC = "stress.e2e.ts";

/** Every scenario spec's file name, in name order; `exclude` still drops the ones run apart. */
function scenarioSpecs(): string[] {
  return readdirSync(join(dirname(fileURLToPath(import.meta.url)), "specs"))
    .filter((name) => name.endsWith(".e2e.ts"))
    .sort();
}

export const config: WebdriverIO.Config = {
  runner: "local",
  // The state specs run separately, against a harness that reports through hooks
  // (`wdio.state.conf.ts`): the app reads `SHELL` once, and one run is one harness.
  //
  // One part of a split run when `PURLIS_E2E_PART` says which ("2/4", CI's `scenario` legs):
  // cut by measured time, not WebdriverIO's equal count (`parts.ts`, #3615). `exclude` still
  // applies to a part, so the specs run apart (below) stay apart.
  //
  // A stress run (STRESS=1) runs the stress spec LAST, after everything the others left open,
  // which is what the stress job says it does. In name order it ran 82nd of 106, and the
  // specs after it ran in an app slowed down by its two hundred chats (#5615). Those specs
  // failed there and nowhere else: theme-gallery on Ubuntu, and workspace-explorer's chat
  // start on macOS (#5586).
  // [LOOP-6232] temporary: only the looped spec.
  specs: process.env.PURLIS_LOOP
    ? ["./specs/artifact-queries.e2e.ts"]
    : process.env.PURLIS_E2E_PART
    ? specsOfPart(scenarioSpecs(), process.env.PURLIS_E2E_PART).map((name) => `./specs/${name}`)
    : process.env.STRESS === "1"
      ? [...scenarioSpecs().filter((name) => name !== STRESS_SPEC), STRESS_SPEC].map(
          (name) => `./specs/${name}`,
        )
      : ["./specs/**/*.e2e.ts"],
  // The state specs run separately (above). The stress spec runs separately too, and for a
  // different reason: it exists to give charter-app#16 — an app that died once while fifty
  // tabs were opening — a chance to recur where the evidence is kept. Fifty windows on a
  // shared CI runner is inherently timing-bound, and it has failed on `main`'s own push and
  // on four of five attempts from an unrelated branch. As a REQUIRED check it stopped being
  // evidence and became a gate that blocks every merge, including fixes for the very bug it
  // watches for. `STRESS=1` includes it; its own CI job runs it and is allowed to fail.
  // The Finder-launch spec runs separately too (`wdio.finder.conf.ts`), and for the reason
  // charter-app#134 hid for as long as it did: it is about the ENVIRONMENT the app is started
  // with, and one run is one environment. Running it here would give it CI's own `PATH` and it
  // would pass without asking anything.
  exclude: [
    "./specs/**/*.state.e2e.ts",
    "./specs/**/*.finder.e2e.ts",
    // The Skill studio's outcome bars are one run per harness (`wdio.outcome.conf.ts`).
    "./specs/**/*.outcome.e2e.ts",
    // The layout-file spec needs its file on disk before the app starts (`wdio.layout.conf.ts`).
    "./specs/**/*.layout.e2e.ts",
    // A project's theme pick needs its file on disk before the app starts (`wdio.picked.conf.ts`).
    "./specs/**/*.picked.e2e.ts",
    // The upgrade spec needs an old install on disk before the app starts, and an app that
    // migrates at launch (`wdio.upgrade.conf.ts`).
    "./specs/**/*.upgrade.e2e.ts",
    // The rail's show and keep hold over its own hide for the launch (`wdio.rail.conf.ts`).
    "./specs/**/*.rail.e2e.ts",
    // A browser view's real engine is downloaded on its run's first browser
    // (`wdio.browser.conf.ts`, #2025).
    "./specs/**/*.browser.e2e.ts",
    // Pool setup's Docker path needs Docker and the runner image this build makes, which only
    // Linux's pool leg has (`wdio.pool.conf.ts`, #4985).
    "./specs/**/*.pool.e2e.ts",
    // The nightly scale run opens a project of a thousand workspaces (`wdio.scale.conf.ts`, #5497).
    "./specs/**/*.scale.e2e.ts",
    ...(process.env.STRESS === "1" ? [] : ["./specs/stress.e2e.ts"]),
  ],
  maxInstances: 1,
  framework: "mocha",
  reporters: ["spec"],
  logLevel: "warn",
  waitforTimeout: 20_000,
  connectionRetryTimeout: 120_000,
  // `leftBehind.ts`'s root hooks run around every spec file, in every config that spreads this
  // one: a file that leaves a chat running, or a tab open on the strip it began with, is cleaned
  // up after and failed by name (#1796, #5111). Resolved against this config's own folder.
  mochaOpts: { ui: "bdd", timeout: 180_000, require: ["./leftBehind.ts"] },

  // Every session starts from a plane with no record in it, so a spec is never handed the
  // chats of the one before it. Each config declares this for ITS OWN plane: `wdio.state.conf`
  // spreads this one and copies a plane of its own, and a hook that closed over the wrong
  // path would quietly clean nothing.
  beforeSession() {
    aSessionBegins();
    anEmptyRecord(plane);
  },

  // Each spec file's whole time, for the run's notice of the slowest ones (`timings.ts`).
  afterSession(_config, _capabilities, specs) {
    aSessionEnds(specs);
  },

  // Every spec starts once the app has drawn its window (#1469): the service's own `before` waits
  // for a window handle, not for the page in it, and the first spec of a cold start used to send
  // its first key to a window with nothing in it yet. Every config spreads this one, so each of
  // them waits too.
  async before() {
    // A press that misses is described as it misses (#5609, `covers.ts`).
    describeEveryMissedPress();
    await theWindowIsDrawn();
    // The default window, wide enough for the project rail beside both sidebars, whatever an
    // earlier spec left (`rail.ts`, #3077). A spec that measures a narrower window, where the
    // rail hides itself to leave the work its room, sets that size itself.
    await theWindowHoldsTheRail();
    // A dialog an earlier spec left up would cover every control of this one, and fifty specs
    // would fail for the one that left it (#1949's scenario stall).
    await noDialogIsLeftUp();
    // The project's folders, accepted once, as the person does at its first open (#3718).
    await theProjectsFoldersAreAccepted();
  },

  // The service reads `tauri:options`, which WebdriverIO's own capability type does not know.
  capabilities: [{ browserName: "tauri", "tauri:options": { application: launched } }] as never,
  services: [
    [
      "@wdio/tauri-service",
      {
        appBinaryPath: launched,
        // The app's own output, for when a scenario test fails because the app did.
        captureBackendLogs: true,
        captureFrontendLogs: true,
        env: environment,
      },
    ],
  ],

  // A miss a spec caught on purpose is never handed to a later failure (`covers.ts`, #5609).
  beforeTest() {
    forgetTheMiss();
  },
  beforeHook() {
    forgetTheMiss();
  },

  // A test that fails because the app died looks, from the test, like any other: an element
  // that never came. So every failure writes down whether the app is alive, its size, and
  // what the operating system and the app's panic hook recorded (charter-app#16).
  async afterTest(test, _context, { passed, error, duration }) {
    recordTiming(test, duration);
    if (!passed) {
      recordFailure(test, error ?? "failed with no error", await evidence(error));
      const where = collectEvidence(app, `${test.parent} ${test.title}`);
      console.error(`charter-e2e: what was left to look at is in ${where}`);
    }
  },

  // A failed `before` or `beforeEach` fails every test under it without an `afterTest`, so the
  // hook's own failure is what gets annotated (`failures.ts`).
  async afterHook(test, _context, { error, duration }) {
    recordTiming(test, duration ?? 0, "hook");
    recordFailure(test, error, error ? await evidence(error) : {});
  },
};
