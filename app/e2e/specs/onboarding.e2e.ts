// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import process from "node:process";
import { $, browser, expect } from "@wdio/globals";
import { press, untilSays } from "../acting.js";
import { aGitAnswerIsWaiting, noGitAnswerWaits, theNextLookForGitFindsNone } from "../gitAnswer.js";
import { THE_RUNS_TREE } from "../harness.js";
import { closeProject } from "../opening.js";
import { ask, inFront, PROJECTS } from "../switching.js";

/**
 * **The guided steps in the built window, from the welcome to an open project, and a resume**
 * (#1817, spec #1810's first seam; and #1813's guided screen, mounted).
 *
 * `src/Onboarding.test.tsx` holds every step against a stood-in core. What only the real window
 * can say:
 *
 * - **the full path**: welcome → Create a project → a repo's path → **the git check** (this
 *   runner has git, so the e2e build's one-look stand-in, `gitAnswer.ts`, has the first look
 *   find none) → Check again, which finds git and carries the open on → **Pick your harness**,
 *   with the new project held behind it → Skip for now → the project in front. "Step N of M"
 *   at each, and the real core's `onboarding.json` keeping each place;
 * - **a resume**: the place the window keeps is the place it comes back to. A cold relaunch is
 *   not reachable from here (one app process serves the run, as `view-tabs.e2e.ts` says), so it
 *   is proved in two halves through the real store: a step the window was at is what the core
 *   keeps, and a place the core keeps — written as a quit would have left it — is the step the
 *   window draws the next time the steps appear.
 *
 * **It leaves the window as it found it**: the local project it made is closed by its own ×
 * and forgotten, the kept place is put back to a machine that has been through the steps, any
 * stand-in nothing used is taken back, and the launch's project is brought back to the front.
 */

const STEPS = 'nav[aria-label="Setting up"]';
const WELCOME = "section.welcome";
const DOORS = 'ul[aria-label="Ways to start"]';
const OPEN_ONE = `${PROJECTS} button[aria-label="Open a project…"]`;
const PICKER = '[role="dialog"][aria-labelledby="start-chat"]';

type Kept = {
  step: string;
  door: string | null;
  signing_in: boolean;
  git_missing: boolean;
  made: { plane: string; cwd: string; workspace: string } | null;
  finished_once: boolean;
};

const FRESH: Kept = {
  step: "welcome",
  door: null,
  signing_in: false,
  git_missing: false,
  made: null,
  finished_once: false,
};

/** A repo with one commit, named `name`, whose `origin` is on github.com and never reached. */
function aRepo(name: string): string {
  const at = join(mkdtempSync(join(THE_RUNS_TREE, "onboarding-")), name);
  mkdirSync(at, { recursive: true });
  const git = (args: string[]) =>
    execFileSync(
      "git",
      ["-c", "user.name=purlis scenario", "-c", "user.email=scenario@example.invalid", ...args],
      {
        cwd: at,
        stdio: "pipe",
        env: { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_SYSTEM: "/dev/null" },
      },
    );
  git(["init", "-q", "-b", "main", "."]);
  writeFileSync(join(at, "README.md"), `# ${name}\n`);
  git(["add", "-A"]);
  git(["commit", "-q", "-m", "start"]);
  git(["remote", "add", "origin", `https://github.com/scenario/${name}.git`]);
  return at;
}

/** Draws the steps from the strip's `+`, pressed from the page, and waits for the doors. */
async function drawTheSteps(): Promise<void> {
  const pressed = await browser.execute((css: string) => {
    const button = document.querySelector<HTMLElement>(css);
    button?.click();
    return button !== null;
  }, OPEN_ONE);
  if (!pressed) throw new Error("the Projects strip offers no Open a project…");
  await $(STEPS).waitForExist({ timeout: 20_000, timeoutMsg: "the steps never came" });
}

/** The level-one heading of the step drawn. */
async function pageHeading(): Promise<string> {
  return browser.execute(
    (css: string) => document.querySelector(`${css} h1`)?.textContent?.trim() ?? "",
    WELCOME,
  );
}

/** Waits until the step drawn is headed `heading`, and has the keyboard on that heading. */
async function untilTheStep(heading: string, timeout = 30_000): Promise<void> {
  await browser.waitUntil(async () => (await pageHeading()) === heading, {
    timeout,
    interval: 250,
    timeoutMsg: `the step ${heading} never came; the page says ${await pageHeading()}`,
  });
  await browser.waitUntil(
    async () =>
      browser.execute(
        (css: string) => document.activeElement === document.querySelector(`${css} h1`),
        WELCOME,
      ),
    { timeout: 10_000, timeoutMsg: `the keyboard never came to ${heading}` },
  );
}

/** What the steps say of where the person is: "Step N of M: name". */
async function stepSaid(): Promise<string> {
  return browser.execute(
    (css: string) =>
      document.querySelector(`${css} .onboarding-count`)?.textContent?.replace(/\s+/g, " ") ?? "",
    STEPS,
  );
}

/** Answers the trust question when it is up, until `done` holds. */
async function answeringTheGateUntil(done: () => Promise<boolean>, why: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      const gate = await $("button=Open project");
      if (await gate.isExisting().catch(() => false)) await gate.click().catch(() => undefined);
      return done();
    },
    { timeout: 90_000, interval: 500, timeoutMsg: why },
  );
}

/** Brings the launch's project to the front from its tab, which puts the steps away. */
async function backTo(first: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      if ((await inFront()) === first && !(await $(STEPS).isExisting())) return true;
      await browser.execute(
        (strip: string, path: string) =>
          [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)]
            .find((tab) => tab.getAttribute("data-directory") === path)
            ?.click(),
        PROJECTS,
        first,
      );
      return false;
    },
    { timeout: 20_000, interval: 250, timeoutMsg: "the launch's project never came back" },
  );
}

describe("the guided steps", function () {
  this.timeout(300_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";
  /** The local project the steps made, once they have. */
  let local = "";

  before(async () => {
    await $(OPEN_ONE).waitForClickable({ timeout: 60_000 });
    first = (await ask<string[]>("open_planes"))[0];
    // A machine that has never been through the steps.
    await ask("keep_onboarding_progress", { progress: FRESH });
  });

  after(async () => {
    noGitAnswerWaits();
    if (
      await $(PICKER)
        .isExisting()
        .catch(() => false)
    )
      await browser.keys(["Escape"]);
    if (local !== "") {
      const closer = `${PROJECTS} button[aria-label="Close project ${basename(local)}"]`;
      if (await $(closer).isExisting()) await closeProject(closer);
    }
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
    if (local !== "") {
      await ask("revoke_approval", { path: local }).catch(() => undefined);
      await ask("forget_project", { path: local }).catch(() => undefined);
    }
    // The specs after this one meet a machine that has been through the steps once.
    await ask("keep_onboarding_progress", { progress: { ...FRESH, finished_once: true } });
    await backTo(first);
  });

  it("go from the welcome through the git check and the harness step to an open project", async () => {
    const repo = aRepo("onboarded");
    const before = await ask<string[]>("open_planes");
    await drawTheSteps();
    expect(await stepSaid()).toContain("Step 1 of 4: Welcome");

    await press(DOORS, "Create a project");
    await untilTheStep("Create a project");
    expect(await stepSaid()).toContain("Step 2 of 4: Create, get or open a project");

    // This runner has git: the window's next look finds none, once.
    theNextLookForGitFindsNone();
    const box = await $(`${WELCOME} input[placeholder="/path/to/repo"]`);
    await box.waitForExist({ timeout: 20_000 });
    await box.addValue(repo);
    await press(WELCOME, "Open repo");

    // **#1813's guided screen**: Check again, and the act that waited carries on.
    await untilTheStep("Install git");
    if (aGitAnswerIsWaiting()) throw new Error("the window never looked for git");
    await untilSays(WELCOME, "Git is not installed");
    expect(await stepSaid()).toContain("Step 3 of 5: Install git");
    expect((await ask<string[]>("open_planes")).length).toBe(before.length);
    await press(WELCOME, "Check again");

    // The project is made, and held behind the harness step.
    await answeringTheGateUntil(
      async () => (await pageHeading()) === "Pick your harness",
      `the harness step never came; the page says ${await pageHeading()}`,
    );
    await untilTheStep("Pick your harness");
    expect(await stepSaid()).toContain("Step 4 of 5: Pick your harness");
    local = (await ask<string[]>("open_planes")).find((one) => !before.includes(one)) ?? "";
    if (local === "") throw new Error("the steps opened no project");
    expect(await inFront()).toBeNull();
    const kept = await ask<Kept>("onboarding_progress");
    expect(kept.step).toBe("harness");
    expect(kept.git_missing).toBe(true);
    expect(kept.made?.plane).toBe(local);
    // The Set up a harness tab's own rows: what is installed, an installer for what is not.
    await untilSays(WELCOME, "Check again");

    // Skip for now still opens the project.
    await press(WELCOME, "Skip for now");
    await browser.waitUntil(async () => (await inFront()) === local, {
      timeout: 30_000,
      timeoutMsg: `the project never came to the front; in front is ${String(await inFront())}`,
    });
    expect(await $(STEPS).isExisting()).toBe(false);
    // The window keeps the finish once it has drawn the project: waited for, so the next test's
    // own place is written after it.
    const finished: Kept = { ...FRESH, finished_once: true };
    let keptLast: Kept | undefined;
    await browser
      .waitUntil(
        async () => {
          keptLast = await ask<Kept>("onboarding_progress");
          return (Object.keys(finished) as (keyof Kept)[]).every(
            (key) => JSON.stringify(keptLast?.[key]) === JSON.stringify(finished[key]),
          );
        },
        { timeout: 20_000, timeoutMsg: "the window never kept the finish" },
      )
      .catch(() => undefined);
    expect(keptLast).toEqual(finished);
  });

  it("come back at the step a quit left them at", async () => {
    await ask("keep_onboarding_progress", { progress: FRESH });
    await drawTheSteps();
    await press(DOORS, "Get a project from your forge");
    await untilTheStep("Get a project from your forge");

    // The window keeps the step it is at.
    await browser.waitUntil(async () => (await ask<Kept>("onboarding_progress")).door === "get", {
      timeout: 10_000,
      timeoutMsg: "the window never kept the step it was at",
    });
    expect(await ask<Kept>("onboarding_progress")).toMatchObject({
      step: "project",
      door: "get",
    });
    await backTo(first);

    // And a place the core keeps, as a quit at the sign-in would have left it, is where the
    // steps come back.
    await ask("keep_onboarding_progress", {
      progress: { ...FRESH, step: "sign-in", signing_in: true },
    });
    await drawTheSteps();
    await untilTheStep("Sign in to GitHub or GitLab");
    expect(await stepSaid()).toContain("Step 2 of 5: Sign in");
    await press(WELCOME, "Back");
    await $(DOORS).waitForExist({ timeout: 20_000, timeoutMsg: "Back never drew the welcome" });
  });
});
