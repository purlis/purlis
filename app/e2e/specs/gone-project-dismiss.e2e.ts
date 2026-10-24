// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, realpathSync, renameSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { anAnswerIsWaiting, noAnswerWaits, theNextFolderPickAnswers } from "../dialogs.js";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { closeProject } from "../opening.js";
import { PROJECTS, ask, inFront, openFromTheStrip } from "../switching.js";

/**
 * **A dismissed "project gone" line stays dismissed for the run, and Locate… lets it go**
 * (#1291, train 74), in the built app against the real core and this run's own machine store.
 *
 * Two projects of this spec's own are opened once, closed and moved on disk, so the machine store
 * lists both as gone and the opener draws a line for each (`GoneProjectNotice`). One is
 * dismissed; the other is never touched and is the spec's proof that the opener has read the
 * store again, so "no line" is never a list that had not arrived yet.
 *
 * - The dismissal outlives the opener: it is put away and drawn again, which reads the store
 *   again, and the dismissed project is still gone there and still not drawn.
 * - It lasts only until its cause changes: the folder put back is no longer gone, and when it
 *   goes again its line is drawn again.
 * - Locate… in Settings › You › This machine re-points the entry, and the dismissal goes with
 *   the old place: a project gone from that place later is drawn again.
 *
 * **Not here: the window's own line** about a project the last quit had open. That line is drawn
 * only for what a launch could not put back, and this run has one launch, made before any spec
 * (`GoneProjects.test.tsx` and `App.test.tsx` hold it against a mocked core). The two lines read
 * one list of dismissals (`GoneProjectNotice.tsx`), which is what the opener half pins here.
 *
 * **Nothing left behind**: one app process serves the whole run, so every project this spec
 * opened is closed, the Settings tab closed by its own ×, an unused folder answer taken back,
 * the store's entries forgotten, and the launch's project put back in front.
 */

const WORKSPACES = '[data-strip="Workspaces"]';
const TABS = '[data-strip="Tabs"]';
const LEVEL = '[role="radiogroup"][aria-label="Level"]';
const OPEN_ONE = `${PROJECTS} button[aria-label="Open a project…"]`;
const PATH_BOX = '[data-setting="open-by-path"] input';

/** The project whose line is dismissed, by the name it is remembered under. */
const NAME = "gone-dismissed";
/** The project whose line is never touched. */
const CONTROL = "gone-control";

/** A project of this spec's own, copied into the run's tree under `name`. */
function aProjectCalled(name: string): string {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), name);
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  // No record to put back, so opening it starts no chat for the next spec to inherit.
  anEmptyRecord(root);
  return root;
}

const mine = aProjectCalled(NAME);
const control = aProjectCalled(CONTROL);
/** Where each is moved to: beside where it was, under another name, as a person moves one. */
const mineMoved = `${mine}-moved`;
const controlMoved = `${control}-moved`;

type Recents = { gone?: { path: string; said: string }[] | null };
type MachineProject = { path: string; name: string; gone: string | null };

/** The paths the core answers are gone, the opener's own question (`recent_planes`). */
async function goneNow(): Promise<string[]> {
  return ((await ask<Recents>("recent_planes")).gone ?? []).map((one) => one.path);
}

/** The path the store holds `name` under while it is gone, waited for. */
async function goneUnder(name: string): Promise<string> {
  let found: string | undefined;
  await browser.waitUntil(
    async () => {
      found = (await goneNow()).find((path) => basename(path) === name);
      return found !== undefined;
    },
    { timeout: 20_000, timeoutMsg: `the store never listed ${name} as gone` },
  );
  return found ?? "";
}

/** Whether the line about `path` is drawn and in view: by its cause, as `Notice` marks it. */
async function line(path: string): Promise<"shown" | "absent"> {
  return browser.execute((cause: string) => {
    const found = [...document.querySelectorAll<HTMLElement>("[data-cause]")].find(
      (one) => one.dataset.cause === cause,
    );
    return found !== undefined && found.closest("[hidden]") === null ? "shown" : "absent";
  }, `project-gone:${path}`);
}

async function untilLine(path: string, want: "shown" | "absent", why: string): Promise<void> {
  await browser
    .waitUntil(async () => (await line(path)) === want, { timeout: 30_000, interval: 250 })
    .catch(async () => {
      // Said in full, because a scenario's failure is read from its annotation.
      throw new Error(
        `${why}: the line about ${path} stayed ${want === "shown" ? "absent" : "drawn"}; the window says: ${await windowSays()}`,
      );
    });
}

/** Draws the opener from the strip's `+`, pressed from the page, and waits for its path box. */
async function drawTheOpener(): Promise<void> {
  const pressed = await browser.execute((css: string) => {
    const button = document.querySelector<HTMLElement>(css);
    button?.click();
    return button !== null;
  }, OPEN_ONE);
  if (!pressed) throw new Error("the Projects strip offers no Open a project…");
  await $(PATH_BOX).waitForExist({ timeout: 20_000, timeoutMsg: "the opener never came" });
}

/** Puts the opener away by bringing the launch's project to the front from its tab. */
async function backTo(first: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      if ((await inFront()) === first && !(await $(PATH_BOX).isExisting())) return true;
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

/** Opens `path` through the strip, as a person does, and closes it again by its ×. */
async function openAndClose(path: string): Promise<void> {
  await openFromTheStrip(path);
  await $(`${WORKSPACES} [role="tab"].plane-root`).waitForExist({ timeout: 30_000 });
  await closeProject(`${PROJECTS} [role="tab"][aria-label="${basename(path)}"]`);
  await browser.waitUntil(async () => !(await ask<string[]>("open_planes")).includes(path), {
    timeout: 20_000,
    timeoutMsg: `${path} stayed open`,
  });
}

/** Presses the Dismiss of the line about `path`, from the page. */
async function dismiss(path: string): Promise<void> {
  const pressed = await browser.execute((cause: string) => {
    const notice = [...document.querySelectorAll<HTMLElement>("[data-cause]")].find(
      (one) => one.dataset.cause === cause,
    );
    const button = notice?.querySelector<HTMLElement>("[data-way=dismiss]");
    button?.click();
    return button !== undefined && button !== null;
  }, `project-gone:${path}`);
  if (!pressed) throw new Error(`the line about ${path} has no Dismiss`);
  await untilLine(path, "absent", "Dismiss did not put it away");
}

describe("a dismissed project-gone line", function () {
  this.timeout(300_000);

  /** The project the launch opened, which this spec never closes. */
  let first = "";
  /** The paths the store holds the two projects under while they are gone. */
  let gone = "";
  let goneControl = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
    // Opened through the opener, as a person opens one, which is what puts it among the
    // recents; moved while nothing holds it.
    await openAndClose(mine);
    renameSync(mine, mineMoved);
    await openAndClose(control);
    renameSync(control, controlMoved);
    await backTo(first);
    gone = await goneUnder(NAME);
    goneControl = await goneUnder(CONTROL);
  });

  after(async () => {
    noAnswerWaits();
    // Each Settings tab this spec opened, closed by its own ×.
    await browser.execute((strip: string) => {
      for (const closer of document.querySelectorAll<HTMLElement>(
        `${strip} button[aria-label^="Close Settings"]`,
      ))
        closer.click();
    }, TABS);
    // Anything the window could not close is let go of in the core all the same.
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
    // The store as it was found: neither project is remembered, wherever it is.
    const ours = [mine, mineMoved, control, controlMoved, gone, goneControl];
    for (const one of (await ask<{ projects: MachineProject[] }>("this_machine")).projects)
      if ([NAME, CONTROL].includes(one.name) || ours.some((path) => samePlace(one.path, path)))
        await ask("forget_project", { path: one.path });
    await backTo(first);
  });

  it("stays dismissed when the opener is put away and drawn again", async () => {
    await drawTheOpener();
    await untilLine(gone, "shown", "the opener drew no line for a project that moved");
    await untilLine(goneControl, "shown", "the opener drew no line for the other one");

    await dismiss(gone);
    expect(await line(goneControl)).toBe("shown");

    // Drawn again, it reads the store again: the other project's line is the sign it has.
    await backTo(first);
    await drawTheOpener();
    await untilLine(goneControl, "shown", "the opener drawn again never read the store");
    expect(await line(gone)).toBe("absent");
    // Still gone in the core, so only the dismissal keeps it off the screen.
    expect(await goneNow()).toContain(gone);
    await backTo(first);
  });

  it("is let go once its cause changes: back, then gone again, it is drawn again", async () => {
    // The folder is back where the store holds it: not gone, so nothing is said about it.
    renameSync(mineMoved, mine);
    await browser.waitUntil(async () => !(await goneNow()).includes(gone), {
      timeout: 20_000,
      timeoutMsg: "the store still says the project put back is gone",
    });
    await drawTheOpener();
    await untilLine(goneControl, "shown", "the opener never read the store");
    expect(await line(gone)).toBe("absent");
    await backTo(first);

    // Gone again is a new occurrence, which a dismissal of the old one does not hide.
    renameSync(mine, mineMoved);
    await drawTheOpener();
    await untilLine(gone, "shown", "a dismissal outlived the cause it was about");
    await backTo(first);
  });

  it("is let go by Locate…, so the old place gone again is drawn again", async () => {
    await drawTheOpener();
    await untilLine(gone, "shown", "the opener drew no line for the project that moved");
    await dismiss(gone);
    await backTo(first);

    // Located in Settings › You › This machine, with the launch's project in front.
    await browser.executeAsync((done: () => void) => {
      void window.__TAURI__.event.emit("settings-asked").then(done, done);
    });
    await $(LEVEL).waitForExist({ timeout: 30_000 });
    await browser.execute((selector: string) => {
      [...document.querySelectorAll<HTMLElement>(`${selector} [role="radio"]`)]
        .find((one) => one.textContent?.trim() === "You")
        ?.click();
    }, LEVEL);
    await browser.waitUntil(
      async () =>
        browser.execute(
          (selector: string) =>
            document
              .querySelector(`${selector} [role="radio"][aria-checked="true"]`)
              ?.textContent?.trim() === "You",
          LEVEL,
        ),
      { timeout: 30_000, timeoutMsg: "Settings never came to the You level" },
    );
    await browser.execute(() =>
      [...document.querySelectorAll<HTMLElement>('nav[aria-label="Groups"] button')]
        .find((one) => one.textContent?.trim() === "This machine")
        ?.click(),
    );
    const locate = `button[aria-label="Locate ${NAME}"]`;
    await $(locate).waitForExist({
      timeout: 30_000,
      timeoutMsg: "This machine never offered Locate… for the project that moved",
    });
    theNextFolderPickAnswers(mineMoved);
    await browser.execute(
      (css: string) => document.querySelector<HTMLElement>(css)?.click(),
      locate,
    );
    await browser
      .waitUntil(async () => !(await goneNow()).includes(gone), { timeout: 30_000 })
      .catch(async () => {
        throw new Error(
          `Locate… never re-pointed ${gone}; the folder pick's answer was ${anAnswerIsWaiting() ? "never read" : "read"}; the window says: ${await windowSays()}`,
        );
      });
    await browser.execute((strip: string) => {
      for (const closer of document.querySelectorAll<HTMLElement>(
        `${strip} button[aria-label^="Close Settings"]`,
      ))
        closer.click();
    }, TABS);

    // The old place made a project again, remembered, and gone once more. Opening it draws the
    // opener, which reads the store after the Locate: the old place is not in it, so its
    // dismissal goes.
    renameSync(mineMoved, mine);
    await openAndClose(mine);
    renameSync(mine, mineMoved);
    await backTo(first);
    await drawTheOpener();
    await untilLine(gone, "shown", "a dismissal outlived the Locate… that re-pointed it");
    await backTo(first);
  });
});

/** What every Notice and status line in the window says, for a failure's message. */
async function windowSays(): Promise<string> {
  return browser.execute(() =>
    [...document.querySelectorAll<HTMLElement>('[data-cause],[role="alert"],[role="status"]')]
      .map((one) => `${one.dataset.cause ?? ""} ${one.innerText.trim()}`.trim())
      .filter(Boolean)
      .join(" | "),
  );
}

/** Whether two paths name one place: as written, or as the file system resolves them. */
function samePlace(one: string, other: string): boolean {
  if (one === "" || other === "") return false;
  if (one === other) return true;
  const real = (path: string) => (existsSync(path) ? realpathSync(path) : path);
  return real(one) === real(other);
}
