// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import { basename, join } from "node:path";
import process from "node:process";
import { $, browser, expect } from "@wdio/globals";
import { press } from "../acting.js";
import { anAnswerIsWaiting, noAnswerWaits, theNextFolderPickAnswers } from "../dialogs.js";
import { THE_RUNS_TREE } from "../harness.js";
import { closeProject } from "../opening.js";
import { ask, inFront, PROJECTS } from "../switching.js";

/**
 * **Opening a folder that is not a project offers to make it one** (#1822, spec #1810 story 28),
 * in the built window.
 *
 * A repo with no manifest anywhere above it is picked through the Open door (the e2e build's
 * folder dialog seam, as `welcome.e2e.ts` does). The window does not refuse it: it says "This
 * folder isn't a purlis project yet. Make it one?" with the template choice.
 *
 * - **Not now** comes back to the doors, opens nothing, and leaves the folder byte for byte as it
 *   was.
 * - **Make it a project** takes the folder into this machine's project, through the trust
 *   question when this machine has not approved that project yet, and the project opens. The
 *   folder is still left as it was: nothing is written into it.
 *
 * `src/MakeItAProject.test.tsx` holds the page's words, the template it sends and the keyboard
 * against a mocked core; `purlis_core::firstrun` holds what taking a repo in writes.
 *
 * **It leaves the window as it found it**: the local project is closed by its own ×, its
 * approval taken back and the store made to forget it, as `opened-repo-forges.e2e.ts` does, and
 * the launch's project is brought back to the front.
 */

const OPEN_ONE = `${PROJECTS} button[aria-label="Open a project…"]`;
const DOORS = 'ul[aria-label="Ways to start"]';
const WELCOME = "section.welcome";
const OPEN_DOOR = "Open a folder on this machine…";
const OFFER = "Make it a project?";
const PICKER = '[role="dialog"][aria-labelledby="start-chat"]';
/** The folder name of the project purlis keeps on this machine (`firstrun::LOCAL_PLANE`). */
const LOCAL_PROJECT = "local-project";

/** A repo with one commit and no manifest, in a folder of the run's tree that is in no project. */
function aPlainRepo(name: string): string {
  const at = join(mkdtempSync(join(THE_RUNS_TREE, "plain-")), name);
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
  return realpathSync(at);
}

/** What a folder holds at its top, by name, with each file's text: what "untouched" compares. */
function whatIsIn(folder: string): string {
  return readdirSync(folder)
    .filter((name) => name !== ".git")
    .sort()
    .map((name) => {
      try {
        return `${name}:${readFileSync(join(folder, name), "utf8")}`;
      } catch {
        return `${name}/`;
      }
    })
    .join("\n");
}

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

/** The level-one heading in the welcome or the page in its place. */
async function pageHeading(): Promise<string> {
  return browser.execute(
    (css: string) => document.querySelector(`${css} h1`)?.textContent?.trim() ?? "",
    WELCOME,
  );
}

/** Picks `folder` through the Open door, and waits for the offer to make it a project. */
async function pickThroughTheOpenDoor(folder: string): Promise<void> {
  theNextFolderPickAnswers(folder);
  await press(DOORS, OPEN_DOOR);
  await browser.waitUntil(async () => (await pageHeading()) === OFFER, {
    timeout: 30_000,
    timeoutMsg: `the offer never came; the page says ${await pageHeading()}, and the folder pick's answer was ${
      anAnswerIsWaiting() ? "never read" : "read"
    }`,
  });
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

describe("a folder that is not a project is offered to be made one", function () {
  this.timeout(300_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";
  /** This machine's project, once Make it a project opened it. */
  let local = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
  });

  after(async () => {
    noAnswerWaits();
    for (let left = 0; left < 3; left++) {
      const up = await $('[role="dialog"]')
        .isExisting()
        .catch(() => false);
      if (!up) break;
      await browser.keys(["Escape"]);
    }
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
    await backTo(first);
  });

  it("leaves the folder untouched and opens nothing on Not now", async () => {
    const folder = aPlainRepo("declined");
    const was = whatIsIn(folder);
    const open = await ask<string[]>("open_planes");
    await drawTheWelcome();

    await pickThroughTheOpenDoor(folder);
    await press(WELCOME, "Not now");

    await $(DOORS).waitForExist({ timeout: 20_000, timeoutMsg: "Not now never drew the doors" });
    expect(await ask<string[]>("open_planes")).toEqual(open);
    expect(whatIsIn(folder)).toEqual(was);
  });

  it("makes it a project, through the trust question, and writes nothing into it", async () => {
    const folder = aPlainRepo("accepted");
    const was = whatIsIn(folder);
    const open = await ask<string[]>("open_planes");
    await drawTheWelcome();

    await pickThroughTheOpenDoor(folder);
    await press(WELCOME, "Make it a project");

    // The trust question, where this machine has not approved its project yet; then the first
    // chat's picker, which is let go: neither is this spec's to answer beyond yes.
    await browser.waitUntil(
      async () => {
        const gate = await $("button=Open project");
        if (await gate.isExisting().catch(() => false)) await gate.click().catch(() => undefined);
        if (
          await $(PICKER)
            .isExisting()
            .catch(() => false)
        )
          await browser.keys(["Escape"]);
        // Done once the offer is gone and this machine's project is open: newly, or already
        // open from an earlier spec, in which case it is the one shown now.
        if ((await pageHeading()) === OFFER) return false;
        return (await ask<string[]>("open_planes")).some(
          (one) => one !== first && (!open.includes(one) || basename(one) === LOCAL_PROJECT),
        );
      },
      { timeout: 90_000, interval: 500, timeoutMsg: "Make it a project never opened a project" },
    );
    local =
      (await ask<string[]>("open_planes")).find(
        (one) => one !== first && (!open.includes(one) || basename(one) === LOCAL_PROJECT),
      ) ?? "";
    expect(local).not.toEqual(folder);
    expect(whatIsIn(folder)).toEqual(was);
  });
});
