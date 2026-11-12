// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { openMenuOn } from "../acting.js";
import { wcagFound } from "../axe.js";
import {
  aDialogOffers,
  answerTheAsk,
  endEveryChat,
  noDialogIsLeftUp,
  openThePicker,
  pressAndStart,
  pressEndChat,
  showView,
} from "../opening.js";
import { closesTheTabsItOpens } from "../reach.js";
import { keyboardToThePage } from "../regionKeys.js";
import { openProjectSettings, settingsAt } from "../settingsTab.js";

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
 * **Nothing is excluded.** A finding is fixed, or filed in M66 with a run as its evidence and
 * named here against its ticket (a {@link Known} its state hands {@link passes}): that state
 * still runs every rule, fails on any other finding or on one more element, and fails once the
 * known one is gone, so the line goes with the fix (#2929: "don't weaken the check"). None is
 * known now: #6043's Label in Name findings are fixed.
 */

/** A finding filed and not fixed yet: the rule, how many elements it names, and its ticket. */
type Known = { rule: string; elements: number; ticket: number };

/** Runs axe over the window as drawn now: nothing found, or exactly the finding `known` names. */
async function passes(known?: Known): Promise<void> {
  const found = await wcagFound();
  expect(found.filter((one) => one.rule !== known?.rule)).toEqual([]);
  if (known === undefined) return;
  const still = found.find((one) => one.rule === known.rule);
  // Gone means fixed: its `Known` goes too, as its ticket says.
  expect({ ticket: known.ticket, still: still !== undefined }).toEqual({
    ticket: known.ticket,
    still: true,
  });
  expect({ ticket: known.ticket, where: still?.where.length }).toEqual({
    ticket: known.ticket,
    where: known.elements,
  });
}

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

/**
 * Runs the palette's row that starts with `words`: F2 with the keyboard off any chat (a terminal
 * takes F2 for itself), and Enter only once that row is the one selected.
 */
async function paletteRow(words: string): Promise<void> {
  await keyboardToThePage();
  await browser.keys(["F2"]);
  await $("#palette-query").waitForDisplayed({ timeout: 20_000 });
  await $("#palette-query").addValue(words);
  await browser.waitUntil(
    async () =>
      browser.execute(
        (said: string) =>
          document
            .querySelector('[role="option"][aria-selected="true"] .palette-title')
            ?.textContent?.startsWith(said) ?? false,
        words,
      ),
    { timeout: 10_000, timeoutMsg: `the palette did not offer ${words}` },
  );
  await browser.keys(["Enter"]);
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
  // that read nothing would leave every state below green.
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

  // And it asks WCAG 2.2 AA alone: the window has no level-one heading, which axe's best-practice
  // `page-has-heading-one` names on every run that is not held to the tags. Two builder calls that
  // each set the run's options did exactly that on the first run of #6037.
  it("asks only the WCAG 2.2 AA rules, none of axe's best practice", async () => {
    const rules = (await wcagFound()).map((one) => one.rule);
    expect(rules).not.toContain("page-has-heading-one");
    expect(rules).not.toContain("region");
  });

  it("passes as the window opens, with no chat in front", async () => {
    await settled();
    await passes();
  });

  it("passes over Start a chat, a dialog", async () => {
    await openThePicker();
    await $('[role="dialog"]').waitForDisplayed({ timeout: 20_000 });
    await settled();
    try {
      await passes();
    } finally {
      await noDialogIsLeftUp();
    }
  });

  it("passes with a chat open", async () => {
    await pressAndStart("New tab");
    await panesDrawn(1);
    await settled();
    await passes();
  });

  // Screen-reader mode (#629, folded into #2929): the palette's row turns it on, every terminal
  // then keeps rows a screen reader reads, and the window still passes with them drawn. Turned
  // off again, the rows go, as the window was for every state after this one.
  it("passes with screen-reader mode on, the chat's terminal drawing rows a reader reads", async () => {
    const rows = '[data-testid="pane"] .xterm-accessibility-tree';
    const drawn = () =>
      browser.execute(
        (selector: string) => document.querySelector(selector)?.childElementCount ?? 0,
        rows,
      );
    await paletteRow("Turn screen-reader mode on");
    try {
      await browser.waitUntil(async () => (await drawn()) > 0, {
        timeout: 10_000,
        timeoutMsg: "screen-reader mode drew no rows in the chat's terminal",
      });
      await settled();
      await passes();
    } finally {
      await noDialogIsLeftUp();
      await paletteRow("Turn screen-reader mode off");
    }
    await browser.waitUntil(async () => (await drawn()) === 0, {
      timeout: 10_000,
      timeoutMsg: "the terminal kept its reader's rows with screen-reader mode off",
    });
  });

  it("passes with the tab split into two panes", async () => {
    await pressAndStart("Split right");
    await panesDrawn(2);
    await settled();
    await passes();
  });

  it("passes over the palette", async () => {
    await browser.keys(["F2"]);
    await $('[role="dialog"][aria-label="Command palette"]').waitForDisplayed({ timeout: 20_000 });
    await settled();
    try {
      await passes();
    } finally {
      await noDialogIsLeftUp();
    }
  });

  it("passes over a question, End chat's", async () => {
    await browser.keys(["F2"]);
    await $("#palette-query").waitForDisplayed({ timeout: 20_000 });
    await $("#palette-query").addValue("End chat");
    // The row's title is the question's (#6227): the question is waited for, and answered, by
    // that name, never by whichever alert dialog is up.
    const ending = await browser.waitUntil(async () => browser.execute(pressEndChat), {
      timeout: 20_000,
      interval: 250,
      timeoutMsg: "the palette offered no End chat",
    });
    // Up before axe reads it: this one is a chat's, so it offers Close.
    await browser.waitUntil(
      async () => browser.execute(aDialogOffers, "alertdialog", ending, "Close", false),
      { timeout: 20_000, timeoutMsg: `the question about ${ending} was never asked` },
    );
    await settled();
    try {
      await passes();
    } finally {
      // Ended, as the question asked: the split's other chat is ended in `after`.
      await answerTheAsk(ending);
    }
  });

  it("passes over a menu, the project's on the rail", async () => {
    await openMenuOn(`${PROJECTS} [role="tab"][aria-selected="true"]`);
    await $('[role="menu"]').waitForDisplayed({ timeout: 20_000 });
    await settled();
    try {
      await passes();
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
    await passes();
    await showView("Updates");
    await settled();
    await passes();
    await showView("Asks");
  });

  it("passes with the Explorer open", async () => {
    await showView("Explorer");
    await settled();
    try {
      await passes();
    } finally {
      await showView("Chats");
    }
  });

  it("passes on Settings, at the Project level and at You", async () => {
    await openProjectSettings();
    await settled();
    await passes();
    await settingsAt("You", "Motion and contrast");
    await settled();
    await passes();
  });

  it("passes on Settings › Theme, the theme gallery", async () => {
    await settingsAt("Project", "Theme");
    await $("[data-theme-card]").waitForExist({ timeout: 30_000 });
    await settled();
    await passes();
  });
});
