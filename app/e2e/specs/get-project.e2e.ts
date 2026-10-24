// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, realpathSync, renameSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { press, typeInto, untilSays } from "../acting.js";
import { anAnswerIsWaiting, noAnswerWaits, theNextFolderPickAnswers } from "../dialogs.js";
import { copyFixturePlane, THE_RUNS_TREE } from "../harness.js";
import { closeProject } from "../opening.js";
import { ask, PROJECTS } from "../switching.js";

/**
 * **Get a project by URL, in the built window** (#1815, spec #1810's first seam).
 *
 * A person presses Get a project from your forge, pastes a project's URL, picks where it goes,
 * and presses Get project: the project is cloned there and opened through the real trust
 * question, against the real machine store. A bad URL and a missing project each say their own
 * sentence with Retry, and the missing one leaves no folder behind.
 *
 * **The forge is a local stand-in**: a bare repository in the run's tree holding a copy of the
 * `daily` fixture project, reached by a `file://` URL that only the `e2e` build accepts (a
 * shipped build takes HTTPS or SSH alone). The core's own tests drive the HTTPS and SSH
 * spellings, the progress, Cancel mid-clone and each reason against the same kind of stand-in
 * (`purlis-core`'s `getproject`).
 *
 * **The native folder dialog is stood in for** through the e2e build's dialog seam
 * (`dialogs.ts`, #1680): Choose a folder… is answered with a folder in the run's tree, so
 * nothing lands in the runner's real `~/Projects`.
 *
 * **Its own project, and nothing of it left behind**: the project is closed by its own ×, the
 * store forgets it, and the launch's project is brought back to the front. The spec starts no
 * chat, so the chats-left guard (#1796) has nothing to end after it.
 */

const OPEN_ONE = `${PROJECTS} button[aria-label="Open a project…"]`;
const DOORS = 'ul[aria-label="Ways to start"]';
const WELCOME = "section.welcome";
const URL_BOX = '[data-setting="get-project-url"] input';
const DOOR = "Get a project from your forge";

/** The project's name, which the clone's folder is named after. */
const NAME = "fetched";

/** git for the stand-in forge: never the machine's own config, so never its signer. */
function git(dir: string, args: string[]): void {
  execFileSync("git", ["-c", "commit.gpgsign=false", ...args], {
    cwd: dir,
    env: {
      ...process.env,
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_AUTHOR_NAME: "fixture",
      GIT_AUTHOR_EMAIL: "fixture@example.invalid",
      GIT_COMMITTER_NAME: "fixture",
      GIT_COMMITTER_EMAIL: "fixture@example.invalid",
    },
    stdio: ["ignore", "ignore", "ignore"],
  });
}

/** The stand-in forge: `forge/acme/<NAME>.git`, a bare copy of the `daily` fixture project. */
const forge = (() => {
  const copied = copyFixturePlane();
  const work = join(dirname(copied), `${NAME}-work`);
  renameSync(copied, work);
  git(work, ["init", "-q", "-b", "main"]);
  git(work, ["add", "-A"]);
  git(work, ["commit", "-q", "-m", "the project"]);
  const bare = join(THE_RUNS_TREE, "forge", "acme", `${NAME}.git`);
  mkdirSync(dirname(bare), { recursive: true });
  git(dirname(bare), ["clone", "-q", "--bare", work, bare]);
  return realpathSync(bare);
})();

/** Where the person picks to keep their projects in this run. */
const projects = (() => {
  const at = join(THE_RUNS_TREE, "got-projects");
  mkdirSync(at, { recursive: true });
  return realpathSync(at);
})();

/** Where the clone lands: the place picked, then the project's name. */
const landed = join(projects, NAME);

type MachineProject = { path: string; name: string; gone: string | null };

/** Draws the welcome from the strip's `+`, and goes through the Get door to its page. */
async function toTheGetPage(): Promise<void> {
  // Still on the page from the test before: Back to the welcome first, as a person would.
  if (await $(URL_BOX).isExisting()) await press(WELCOME, "Back");
  const pressed = await browser.execute((css: string) => {
    const button = document.querySelector<HTMLElement>(css);
    button?.click();
    return button !== null;
  }, OPEN_ONE);
  if (!pressed) throw new Error("the Projects strip offers no Open a project…");
  await $(DOORS).waitForExist({ timeout: 20_000, timeoutMsg: "the welcome never came" });
  await press(DOORS, DOOR);
  await $(URL_BOX).waitForExist({ timeout: 20_000, timeoutMsg: "the Get page drew no URL box" });
}

/** Picks the run's projects folder with Choose a folder…, and waits for the page to show it. */
async function chooseTheRunsFolder(): Promise<void> {
  theNextFolderPickAnswers(projects);
  await press(WELCOME, "Choose a folder…");
  await untilSays(WELCOME, projects);
  expect(anAnswerIsWaiting()).toBe(false);
}

/** Waits until the cloned project is open, or no longer is. */
async function untilOpen(open: boolean, why: string): Promise<void> {
  await browser.waitUntil(
    async () => (await ask<string[]>("open_planes")).includes(landed) === open,
    { timeout: 60_000, timeoutMsg: why },
  );
}

/** Brings the launch's project to the front from its tab, which puts the welcome away. */
async function backTo(first: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      if (!(await $(DOORS).isExisting()) && !(await $(URL_BOX).isExisting())) return true;
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

describe("getting a project by its URL", function () {
  this.timeout(240_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
  });

  after(async () => {
    noAnswerWaits();
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
    for (const one of (await ask<{ projects: MachineProject[] }>("this_machine")).projects)
      if (one.path === landed) await ask("forget_project", { path: one.path });
    await backTo(first);
  });

  it("says a URL that is not HTTPS or SSH is not one, with Retry, and clones nothing", async () => {
    await toTheGetPage();

    await typeInto(URL_BOX, "http://example.invalid/acme/widget");
    await press(WELCOME, "Get project");

    await untilSays(`${WELCOME} [role="alert"]`, "HTTPS or SSH");
    expect(await $(`${WELCOME} [role="alert"]`).$("button=Retry").isExisting()).toBe(true);
  });

  it("says a project the forge does not have is missing, and leaves no folder", async () => {
    await toTheGetPage();
    await chooseTheRunsFolder();

    await typeInto(URL_BOX, `file://${join(dirname(forge), "nope.git")}`);
    await untilSays(WELCOME, join(projects, "nope"));
    await press(WELCOME, "Get project");

    await untilSays(`${WELCOME} [role="alert"]`, "There is no project at");
    expect(await $(`${WELCOME} [role="alert"]`).$("button=Retry").isExisting()).toBe(true);
    expect(existsSync(join(projects, "nope"))).toBe(false);
  });

  it("clones a pasted URL into the folder chosen, and opens it through the trust question", async () => {
    await toTheGetPage();
    // The place chosen last time is remembered: the page already shows it.
    await typeInto(URL_BOX, `file://${forge}`);
    await untilSays(WELCOME, landed);

    await press(WELCOME, "Get project");
    const asking = await $('[role="dialog"]');
    await asking.waitForExist({
      timeout: 60_000,
      timeoutMsg: `the trust question never came; the clone ${
        existsSync(landed) ? "landed" : "did not land"
      } in ${landed}`,
    });
    expect(existsSync(join(landed, ".git"))).toBe(true);
    await asking.$("button=Open project").click();
    await untilOpen(true, "the cloned project never opened");

    await closeProject(`${PROJECTS} [role="tab"][aria-label="${basename(landed)}"]`);
    await untilOpen(false, "the cloned project stayed open");
  });
});
