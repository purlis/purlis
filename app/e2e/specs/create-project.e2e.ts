// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, realpathSync } from "node:fs";
import { basename, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { press, typeInto, untilSays } from "../acting.js";
import { anAnswerIsWaiting, noAnswerWaits, theNextFolderPickAnswers } from "../dialogs.js";
import { THE_RUNS_TREE } from "../harness.js";
import { closeProject } from "../opening.js";
import {
  aForgeThatCreates,
  holdThePush,
  letThePushGo,
  pushedCommits,
  thePushIsHeld,
} from "../standInForge.js";
import { ask, PROJECTS } from "../switching.js";

/**
 * **Create a project on a forge, in the built window** (#1819, spec #1810's first seam).
 *
 * A person presses Create a project, picks GitHub, names the project and presses Create
 * project: the repo is made on the forge, the project laid out and committed, the first commit
 * pushed, and the project opened through the real trust question. Pressed again with Cancel
 * once the repo is made and before the push, nothing is left: no folder, and the repo deleted.
 *
 * **The forge is a stand-in** (`standInForge.ts`): the e2e build answers the owner picker and
 * the creation from recordings in the forge contract's shape, and pushes to a bare repository
 * in the run's tree, so no spec makes a repo anywhere real. The core's own tests hold every
 * stage's failure and every stage's Cancel against the contract's recordings on both forges
 * (`purlis-core`'s `createproject`), and the real push through purlis's credential helper is
 * the helper's own test (#752).
 *
 * **The native folder dialog is stood in for** (`dialogs.ts`, #1680): Choose a folder… answers a
 * folder in the run's tree, so nothing lands in the runner's real `~/Projects`.
 *
 * **Its own project, and nothing of it left behind**: the made project is closed by its own ×
 * and forgotten, and the launch's project is brought back to the front.
 */

const OPEN_ONE = `${PROJECTS} button[aria-label="Open a project…"]`;
const DOORS = 'ul[aria-label="Ways to start"]';
const WELCOME = "section.welcome";
const WHERE = '[data-setting="create-project-where"]';
const NAME_BOX = '[data-setting="create-project-name"] input';
const OWNER = '[data-setting="create-project-owner"] [role="combobox"]';

/** Where the person keeps their projects in this run. */
const projects = (() => {
  const at = join(THE_RUNS_TREE, "created-projects");
  mkdirSync(at, { recursive: true });
  return realpathSync(at);
})();

type MachineProject = { path: string; name: string; gone: string | null };

/** Draws the welcome from the strip's `+`, goes through the Create door, and picks GitHub. */
async function toCreateOnGitHub(): Promise<void> {
  if (await $(NAME_BOX).isExisting()) await press(WELCOME, "Back");
  const pressed = await browser.execute((css: string) => {
    const button = document.querySelector<HTMLElement>(css);
    button?.click();
    return button !== null;
  }, OPEN_ONE);
  if (!pressed) throw new Error("the Projects strip offers no Open a project…");
  await $(DOORS).waitForExist({ timeout: 20_000, timeoutMsg: "the welcome never came" });
  await press(DOORS, "Create a project");
  await $(`${WHERE} button[value="github"]`).waitForExist({
    timeout: 20_000,
    timeoutMsg: "Create a project asked no Where it lives",
  });
  await $(`${WHERE} button[value="github"]`).click();
  // The owners come from the stand-in forge's account: the account itself is picked.
  await browser.waitUntil(
    async () =>
      (await $(OWNER).isExisting()) && (await $(OWNER).getAttribute("data-value")) === "octocat",
    {
      timeout: 20_000,
      timeoutMsg: "the owner picker never offered the account",
    },
  );
}

/** Names the project and picks the run's folder for it, and waits for the page to show where. */
async function nameIt(name: string): Promise<string> {
  await typeInto(NAME_BOX, name);
  theNextFolderPickAnswers(projects);
  await press(WELCOME, "Choose a folder…");
  const folder = join(projects, name);
  await untilSays(WELCOME, folder);
  expect(anAnswerIsWaiting()).toBe(false);
  return folder;
}

/** Brings the launch's project to the front from its tab, which puts the welcome away. */
async function backTo(first: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      if (!(await $(DOORS).isExisting()) && !(await $(NAME_BOX).isExisting())) return true;
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

describe("creating a project on a forge", function () {
  this.timeout(240_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";
  const made: string[] = [];

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
  });

  after(async () => {
    noAnswerWaits();
    letThePushGo();
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
    for (const one of (await ask<{ projects: MachineProject[] }>("this_machine")).projects)
      if (made.includes(one.path)) await ask("forget_project", { path: one.path });
    await backTo(first);
  });

  it("creates the repo, pushes the first commit, and opens the project through the trust question", async () => {
    aForgeThatCreates("garden");
    await toCreateOnGitHub();
    const folder = await nameIt("garden");
    made.push(folder);

    await press(WELCOME, "Create project");
    const asking = await $('[role="dialog"]');
    await asking.waitForExist({
      timeout: 90_000,
      timeoutMsg: `the trust question never came; the project ${
        existsSync(folder) ? "was laid out" : "was not laid out"
      } in ${folder}`,
    });
    // The first commit reached the forge's repository before the project was opened.
    expect(pushedCommits()).toEqual(["Start garden with purlis"]);
    expect(existsSync(join(folder, ".git"))).toBe(true);
    await asking.$("button=Open project").click();
    await browser.waitUntil(async () => (await ask<string[]>("open_planes")).includes(folder), {
      timeout: 60_000,
      timeoutMsg: "the made project never opened",
    });

    await closeProject(`${PROJECTS} button[aria-label="Close project ${basename(folder)}"]`);
    await browser.waitUntil(async () => !(await ask<string[]>("open_planes")).includes(folder), {
      timeout: 60_000,
      timeoutMsg: "the made project stayed open",
    });
  });

  it("takes back the repo and the folder when the person cancels mid-way", async () => {
    aForgeThatCreates("orchard");
    holdThePush();
    await toCreateOnGitHub();
    const folder = await nameIt("orchard");

    await press(WELCOME, "Create project");
    // The repo is made and the push is held: the stage the person is told is the push.
    await untilSays(WELCOME, "Pushing the first commit…");
    expect(existsSync(folder)).toBe(true);
    await press(WELCOME, "Cancel");

    await untilSays(WELCOME, "Cancelled. Nothing was left behind.");
    expect(existsSync(folder)).toBe(false);
    expect(pushedCommits()).toEqual([]);
    expect(thePushIsHeld()).toBe(true);
    letThePushGo();
  });
});
