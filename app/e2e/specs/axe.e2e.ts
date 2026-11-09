// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { openMenuOn } from "../acting.js";
import { wcagFound } from "../axe.js";
import {
  answerTheAsk,
  endEveryChat,
  noDialogIsLeftUp,
  openThePicker,
  pressAndStart,
  showView,
} from "../opening.js";
import { closesTheTabsItOpens } from "../reach.js";
import { openProjectSettings } from "../settingsTab.js";

/**
 * **The whole window held to WCAG 2.2 AA by axe, state by state** (#2929, spec #2897, N35), in
 * the built app: the window as it opens, a chat open, two panes side by side, the Inbox, the
 * Explorer, Settings and its theme gallery, and one dialog of each kind over the shell (a dialog,
 * a question, the palette and a menu).
 *
 * Each story is already held to the same tags in Chromium and WebKit; what is asked here is what
 * only the components together make: two landmarks of a kind with one name, a control a region
 * draws over another, a colour on a ground no story draws. A state that fails says each rule axe
 * named, how bad, what it asks and the first places it found it (`axe.ts`).
 *
 * **Nothing is excluded.** A finding is fixed, or filed in M66 with this run as its evidence and
 * the state's test left red until it is (#2929: "don't weaken the check").
 */

const PANES = '[data-testid="pane"]';
const PROJECTS = '[data-strip="Projects"]';

/** Waits until `count` panes are drawn in the tab in front, each with its terminal. */
async function panesDrawn(count: number): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser
        .execute(
          (panes: string) =>
            [...document.querySelectorAll<HTMLElement>(panes)].filter(
              (pane) => pane.getBoundingClientRect().width > 0 && pane.querySelector("textarea"),
            ).length,
          PANES,
        )
        .then((drawn) => drawn >= count),
    { timeout: 30_000, interval: 250, timeoutMsg: `${count} panes were never drawn` },
  );
}

/** Settles the window before axe reads it: a run during a view's arrival reads it half drawn. */
async function settled(): Promise<void> {
  await browser.execute(
    () =>
      new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))),
  );
  await browser.pause(300);
}

/** Shows the Settings tab's `group` at `level`, as `settings-limits.e2e.ts` does. */
async function settingsAt(level: string, group: string): Promise<void> {
  await browser.execute((called: string) => {
    const radio = [
      ...document.querySelectorAll<HTMLInputElement>(
        '[role="radiogroup"][aria-label="Level"] input[type="radio"]',
      ),
    ].find((one) => one.closest("label")?.textContent?.trim() === called);
    if (radio?.checked !== true) radio?.click();
  }, level);
  await browser.waitUntil(
    async () =>
      browser.execute((called: string) => {
        const button = [
          ...document.querySelectorAll<HTMLElement>('nav[aria-label="Groups"] button'),
        ].find((one) => one.textContent?.trim() === called);
        button?.click();
        return button?.getAttribute("aria-current") === "true";
      }, group),
    { timeout: 20_000, interval: 250, timeoutMsg: `Settings never showed ${level} › ${group}` },
  );
}

describe("the window, held to WCAG 2.2 AA by axe", function () {
  this.timeout(180_000);

  before(async () => {
    await noDialogIsLeftUp();
  });

  after(async () => {
    await noDialogIsLeftUp();
    await showView("Chats").catch(() => undefined);
    await endEveryChat();
  });

  closesTheTabsItOpens();

  // The check can fail: a control axe must name, planted in the window and taken out again. A run
  // that read nothing, or a rule set that lost its tags, would leave every state below green.
  it("finds a nameless button planted in the window", async () => {
    await browser.execute(() => {
      const planted = document.createElement("button");
      planted.id = "e2e-axe-planted";
      document.body.appendChild(planted);
    });
    try {
      const found = await wcagFound();
      const named = found.find((one) => one.rule === "button-name");
      expect(named?.where.some((where) => where.includes("e2e-axe-planted"))).toBe(true);
    } finally {
      await browser.execute(() => document.getElementById("e2e-axe-planted")?.remove());
    }
  });

  it("passes as the window opens, with no chat in front", async () => {
    await settled();
    expect(await wcagFound()).toEqual([]);
  });

  it("passes over Start a chat, a dialog", async () => {
    await openThePicker();
    await $('[role="dialog"]').waitForDisplayed({ timeout: 20_000 });
    await settled();
    try {
      expect(await wcagFound()).toEqual([]);
    } finally {
      await noDialogIsLeftUp();
    }
  });

  it("passes with a chat open", async () => {
    await pressAndStart("New tab");
    await panesDrawn(1);
    await settled();
    expect(await wcagFound()).toEqual([]);
  });

  it("passes with the tab split into two panes", async () => {
    await pressAndStart("Split right");
    await panesDrawn(2);
    await settled();
    expect(await wcagFound()).toEqual([]);
  });

  it("passes over the palette", async () => {
    await browser.keys(["F2"]);
    await $('[role="dialog"][aria-label="Command palette"]').waitForDisplayed({ timeout: 20_000 });
    await settled();
    try {
      expect(await wcagFound()).toEqual([]);
    } finally {
      await noDialogIsLeftUp();
    }
  });

  it("passes over a question, End chat's", async () => {
    await browser.keys(["F2"]);
    await $("#palette-query").waitForDisplayed({ timeout: 20_000 });
    await $("#palette-query").addValue("End chat");
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const row = document.querySelector<HTMLElement>(
            '[id^="palette-row-chat.end:"]:not([aria-disabled="true"])',
          );
          row?.click();
          return row !== null;
        }),
      { timeout: 20_000, interval: 250, timeoutMsg: "the palette offered no End chat" },
    );
    await $('[role="alertdialog"]').waitForDisplayed({ timeout: 20_000 });
    await settled();
    try {
      expect(await wcagFound()).toEqual([]);
    } finally {
      // Ended, as the question asked: the split's other chat is ended in `after`.
      await answerTheAsk("");
    }
  });

  it("passes over a menu, the project's on the rail", async () => {
    await openMenuOn(`${PROJECTS} [role="tab"][aria-selected="true"]`);
    await $('[role="menu"]').waitForDisplayed({ timeout: 20_000 });
    await settled();
    try {
      expect(await wcagFound()).toEqual([]);
    } finally {
      await noDialogIsLeftUp();
      if (
        await $('[role="menu"]')
          .isDisplayed()
          .catch(() => false)
      )
        await browser.keys("Escape");
    }
  });

  it("passes with the Inbox open", async () => {
    await showView("Asks");
    await settled();
    expect(await wcagFound()).toEqual([]);
    await showView("Updates");
    await settled();
    expect(await wcagFound()).toEqual([]);
    await showView("Asks");
  });

  it("passes with the Explorer open", async () => {
    await showView("Explorer");
    await settled();
    try {
      expect(await wcagFound()).toEqual([]);
    } finally {
      await showView("Chats");
    }
  });

  it("passes on Settings, at the Project level and at You", async () => {
    await openProjectSettings();
    await settled();
    expect(await wcagFound()).toEqual([]);
    await settingsAt("You", "Motion and contrast");
    await settled();
    expect(await wcagFound()).toEqual([]);
  });

  it("passes on Settings › Theme, the theme gallery", async () => {
    await settingsAt("Project", "Theme");
    await $("[data-theme-card]").waitForExist({ timeout: 30_000 });
    await settled();
    expect(await wcagFound()).toEqual([]);
  });
});
