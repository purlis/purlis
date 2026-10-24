// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { realpathSync, renameSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { press, untilSays } from "../acting.js";
import { anAnswerIsWaiting, noAnswerWaits, theNextFolderPickAnswers } from "../dialogs.js";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { closeProject } from "../opening.js";
import { ask, inFront, PROJECTS } from "../switching.js";

/**
 * **The welcome's three doors, its sign-in and its recents, in the built window** (#1812, spec
 * #1810's first seam).
 *
 * `src/Welcome.test.tsx` holds what each machine state draws (a fresh store, a gone row, a
 * dropped row, no store, a launch from a terminal) against a mocked core. What only the real
 * window can say is that each door goes where it says in the app as built — its page drawn, the
 * keyboard put on that page's heading and handed back to the door by Back — and that the Open
 * door's folder dialog and a recent row each open a real project through the real trust gate,
 * against the real machine store.
 *
 * **The window shows the welcome from the strip's `+`**: this run's window holds the launch's
 * project, and closing it would end the chats every spec after this one shares an app process
 * with. So this is the welcome a window with projects draws ("Add another project"), which is
 * the same component with the same doors; its other headings are the unit tests'.
 *
 * **The native folder dialog is the one thing stood in for**, through the e2e build's dialog
 * seam (`dialogs.ts`, #1680): the folder a person would have picked. Its answer is read once
 * and removed, so a build that never opened the dialog fails here saying so.
 *
 * **Its own project, and nothing of it left behind**: one app process serves the whole run, so
 * the project is closed by its own ×, any answer nothing used is taken back, the store forgets
 * it, and the launch's project is brought back to the front. The spec starts no chat, so the
 * chats-left guard (#1796) has nothing to end after it.
 */

const OPEN_ONE = `${PROJECTS} button[aria-label="Open a project…"]`;
const DOORS = 'ul[aria-label="Ways to start"]';
const WELCOME = "section.welcome";

/** The doors, by the names a screen reader reads, in the order they are drawn. */
const DOOR_NAMES = [
  "Create a project",
  "Get a project from your forge",
  "Open a folder on this machine…",
];
const SIGN_IN = "Sign in to GitHub or GitLab";

/** The name this spec's project is remembered by. */
const NAME = "welcomed";

/** This spec's own project, copied into the run's tree under {@link NAME}. */
const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), NAME);
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  // Nothing to put back, so opening it starts no chat for the next spec to inherit.
  anEmptyRecord(root);
  return root;
})();

type MachineProject = { path: string; name: string; gone: string | null };

/** Draws the welcome from the strip's `+`, pressed from the page, and waits for its doors. */
async function drawTheWelcome(): Promise<void> {
  const pressed = await browser.execute((css: string) => {
    const button = document.querySelector<HTMLElement>(css);
    button?.click();
    return button !== null;
  }, OPEN_ONE);
  if (!pressed) throw new Error("the Projects strip offers no Open a project…");
  await $(DOORS).waitForExist({ timeout: 20_000, timeoutMsg: "the welcome never came" });
}

/** The doors drawn, each with its name, its description and whether Tab reaches it. */
async function doorsDrawn(): Promise<{ name: string; says: string; tab: number }[]> {
  return browser.execute((css: string) => {
    const list = document.querySelector(css);
    return [...(list?.querySelectorAll<HTMLButtonElement>("button") ?? [])].map((one) => ({
      name: one.getAttribute("aria-label") ?? "",
      says: document.getElementById(one.getAttribute("aria-describedby") ?? "")?.textContent ?? "",
      tab: one.tabIndex,
    }));
  }, DOORS);
}

/** The level-one heading in the welcome or a door's page, read in one pass. */
async function pageHeading(): Promise<string> {
  return browser.execute(
    (css: string) => document.querySelector(`${css} h1`)?.textContent?.trim() ?? "",
    WELCOME,
  );
}

/** What has the keyboard: its accessible name, or its words. */
async function whereTheKeyboardIs(): Promise<string> {
  return browser.execute(() => {
    const at = document.activeElement as HTMLElement | null;
    return (at?.getAttribute("aria-label") ?? at?.textContent ?? "").trim();
  });
}

/** Presses a door, by its name, and waits for its page's heading to hold the keyboard. */
async function throughTheDoor(door: string, heading: string): Promise<void> {
  await press(DOORS, door);
  await browser.waitUntil(async () => (await pageHeading()) === heading, {
    timeout: 20_000,
    timeoutMsg: `${door} never drew ${heading}; the page says ${await pageHeading()}`,
  });
  await browser.waitUntil(async () => (await whereTheKeyboardIs()) === heading, {
    timeout: 10_000,
    timeoutMsg: `the keyboard never came to ${heading}; it is on ${await whereTheKeyboardIs()}`,
  });
}

/** Presses Back on a door's page and waits for the keyboard to be on `door` again. */
async function backToTheDoor(door: string): Promise<void> {
  await press(WELCOME, "Back");
  await $(DOORS).waitForExist({ timeout: 20_000, timeoutMsg: "Back never drew the welcome" });
  await browser.waitUntil(async () => (await whereTheKeyboardIs()) === door, {
    timeout: 10_000,
    timeoutMsg: `Back left the keyboard on ${await whereTheKeyboardIs()}, not on ${door}`,
  });
}

/** Waits until this spec's project is open, or no longer is. */
async function untilOpen(open: boolean, why: string): Promise<void> {
  await browser.waitUntil(
    async () => (await ask<string[]>("open_planes")).includes(mine) === open,
    {
      timeout: 30_000,
      timeoutMsg: why,
    },
  );
}

/** Closes this spec's project by its own ×, as a person does. */
async function closeMine(): Promise<void> {
  await closeProject(`${PROJECTS} [role="tab"][aria-label="${basename(mine)}"]`);
  await untilOpen(false, "this spec's project stayed open");
}

/** Brings the launch's project to the front from its tab, which puts the welcome away. */
async function backTo(first: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      if ((await inFront()) === first && !(await $(DOORS).isExisting())) return true;
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

describe("the welcome's doors and recents", function () {
  this.timeout(240_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
  });

  after(async () => {
    noAnswerWaits();
    // Anything the window could not close is let go of in the core all the same.
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
    // The store as it was found: this spec's project is not remembered.
    for (const one of (await ask<{ projects: MachineProject[] }>("this_machine")).projects)
      if (one.name === NAME || one.path === mine) await ask("forget_project", { path: one.path });
    await backTo(first);
  });

  it("offers three named doors and the sign-in, each one Tab reaches", async () => {
    await drawTheWelcome();

    const doors = await doorsDrawn();
    expect(doors.map((one) => one.name)).toEqual(DOOR_NAMES);
    // Every door says what it does, and WebKit puts it in the tab sequence (`tabindex="0"`).
    for (const door of doors) {
      expect(door.says.length).toBeGreaterThan(0);
      expect(door.tab).toBe(0);
    }
    const signIn = await browser.execute(
      (css: string, name: string) =>
        [...document.querySelectorAll<HTMLButtonElement>(`${css} button`)].find(
          (one) => one.textContent?.trim() === name,
        )?.tabIndex ?? null,
      WELCOME,
      SIGN_IN,
    );
    expect(signIn).toBe(0);
  });

  it("goes to Create a project, and Back hands the keyboard to its door", async () => {
    await drawTheWelcome();

    await throughTheDoor("Create a project", "Create a project");
    await untilSays(WELCOME, "Open a repo…");
    await backToTheDoor("Create a project");
  });

  it("goes to Get a project from your forge, which asks for the project's URL", async () => {
    await drawTheWelcome();

    await throughTheDoor("Get a project from your forge", "Get a project from your forge");
    // The clone itself is `get-project.e2e.ts`'s (#1815).
    await untilSays(WELCOME, "Project URL");
    await backToTheDoor("Get a project from your forge");
  });

  it("offers the browser sign-in beside the token, and lists the forge tools", async () => {
    await drawTheWelcome();

    await press(WELCOME, SIGN_IN);
    await browser.waitUntil(async () => (await pageHeading()) === SIGN_IN, {
      timeout: 20_000,
      timeoutMsg: `Sign in never drew its page; the page says ${await pageHeading()}`,
    });
    // The browser sign-ins on github.com and gitlab.com and the token, side by side (#729,
    // #730). None is pressed: a scenario run never reaches a real forge.
    await untilSays(WELCOME, "Sign in to github.com or gitlab.com through your browser");
    await untilSays(WELCOME, "Sign in with GitHub");
    await untilSays(WELCOME, "Sign in with GitLab");
    await untilSays(WELCOME, "Sign in with a token");
    // The machine was read: each forge tool has its row, whatever this runner has installed.
    await untilSays(WELCOME, "gh:");
    await untilSays(WELCOME, "glab:");
    await backToTheDoor(SIGN_IN);
  });

  it("opens a folder through the Open door and the trust question", async () => {
    await drawTheWelcome();

    theNextFolderPickAnswers(mine);
    await press(DOORS, "Open a folder on this machine…");
    const asking = await $('[role="dialog"]');
    await asking.waitForExist({
      timeout: 30_000,
      timeoutMsg: `the trust question never came; the folder pick's answer was ${
        anAnswerIsWaiting() ? "never read" : "read"
      }`,
    });
    await asking.$("button=Open project").click();
    await untilOpen(true, "the folder the Open door picked never opened");

    await closeMine();
  });

  it("lists that project among the recents, and opens it again from its row", async () => {
    await drawTheWelcome();

    const recents = "section.welcome-recents";
    await untilSays(recents, NAME);
    await press(recents, NAME, "starting");
    // Approved a moment ago, so it opens with no question this time.
    await untilOpen(true, "the recent row never opened its project");
    expect(await $('[role="dialog"]').isExisting()).toBe(false);

    await closeMine();
  });
});
