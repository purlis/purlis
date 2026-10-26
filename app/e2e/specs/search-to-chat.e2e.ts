// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import {
  choose,
  closeTab,
  focusOn,
  keyWithFallback,
  tabTexts,
  textOf,
  typeInto,
} from "../acting.js";
import { READY } from "../harness.js";
import { showView } from "../opening.js";
import { endChatsStartedHere, startAChat, type Started } from "../started.js";

/**
 * **Search is a side view, and Shift+Enter hands a hit to a chat** (#1701, #1151; c0f907ef,
 * d722e653), in the built app against the real core.
 *
 * Search is opened from its icon on the activity bar (the palette's catalogue is another lane's
 * this round), and it is drawn in the side and nowhere on the strip: no Search tab. A query for
 * `svc` finds the fixture clone's `README.md`; Shift+Enter on the hits opens the chat picker for
 * the hit stepped to, and the chat picked has the reference typed into it, unsent, which the
 * view's status line says.
 *
 * **Shift+Enter is asserted by what the app did with it.** The driver's keys are untrusted
 * events, which the hits' own key handler hears like any other; where the driver's Shift+Enter
 * opened no picker, the same key is dispatched to the hits from the page (`acting.ts`).
 *
 * **It leaves the window as it found it**: the query is emptied, the chat it started is ended,
 * and the left side shows Chats again.
 */

const BOX = 'input[aria-label="Search the files"]';
const SCOPE = '[data-scope="select"][data-part="root"][aria-label="Where to search"]';
const HITS = '[role="listbox"][aria-label="Search results"]';
// The chats to pick from are a list of the family (#2911).
const PICKER = '.search-pick [role="listbox"]';
const NAVIGATION = '[role="tablist"][aria-label="Navigation"]';

/** What the status line says was refused, sentence by sentence, read in one pass. */
const refusals = () =>
  browser.execute(() =>
    [...document.querySelectorAll(".search-summary .search-refused")].map((one) =>
      (one.textContent ?? "").trim(),
    ),
  );

describe("Search as a side view, handing a hit to a chat", function () {
  this.timeout(180_000);

  let wereAlreadyOpen: string[] = [];
  /** The chat this spec started, by the number its start answered: an earlier spec may have
   *  left a chat of the same name, and the picker's rows carry each chat's number. */
  let started: Started | undefined;

  before(async () => {
    wereAlreadyOpen = await tabTexts();
    // A chat to hand the hit to.
    started = await startAChat();
    await browser.waitUntil(
      async () =>
        browser.execute(
          (n: number, ready: string) =>
            (
              document.querySelector(`[data-testid="pane"][data-session="${n}"] .xterm-rows`)
                ?.textContent ?? ""
            ).includes(ready),
          started?.session ?? -1,
          READY,
        ),
      { timeout: 30_000, interval: 250, timeoutMsg: "the chat never started" },
    );
    await showView("Search");
    await $(BOX).waitForExist({ timeout: 20_000 });
  });

  // Each step is tried even when the one before it failed: one app process serves the run.
  after(async () => {
    try {
      if (await $(BOX).isExisting()) await typeInto(BOX, "");
    } catch (trouble) {
      console.warn("search-to-chat: the query was not emptied", trouble);
    }
    // A view tab this spec opened (`Files · svc`, where the driver's Enter opened the hit before
    // the page's Shift+Enter reached it) is closed, and the chat it started is ended.
    try {
      for (const name of (await tabTexts()).filter((tab) => !wereAlreadyOpen.includes(tab)))
        if (name.includes(" · ")) await closeTab(name);
    } catch (trouble) {
      console.warn("search-to-chat: a view tab it opened was not closed", trouble);
    }
    try {
      await endChatsStartedHere([started]);
    } finally {
      await showView("Chats");
    }
  });

  it("is drawn in the side, from its icon, and never as a tab on the strip", async () => {
    expect(
      await browser.execute(
        (bar: string) =>
          document
            .querySelector(`${bar} [role="tab"][aria-label="Search"]`)
            ?.getAttribute("aria-selected") ?? null,
        NAVIGATION,
      ),
    ).toBe("true");
    expect(await tabTexts()).not.toContain("Search");
    // The box is in the side's view, not in a pane of the strip.
    expect(
      await browser.execute(
        (box: string) => document.querySelector(box)?.closest('[data-strip="Tabs"]') ?? null,
        BOX,
      ),
    ).toBeNull();
  });

  it("hands the hit stepped to into the chat picked, with its line, sending nothing", async () => {
    await typeInto(BOX, "svc");
    await choose(SCOPE, "project");
    await browser.waitUntil(
      async () =>
        browser.execute(
          (hits: string) => document.querySelectorAll(`${hits} [role="option"]`).length > 0,
          HITS,
        ),
      { timeout: 30_000, interval: 250, timeoutMsg: "the search never found the fixture's svc" },
    );
    const first = await textOf(`${HITS} [role="group"]`);
    expect(first).toContain("README.md");

    await focusOn(HITS);
    await keyWithFallback(["Shift", "Enter"], { key: "Enter", shiftKey: true }, async () =>
      $(PICKER).isExisting(),
    );

    await $(PICKER).waitForExist({ timeout: 10_000 });
    // The picker is for the hit stepped to, the first, with its line.
    expect(await $(PICKER).getAttribute("aria-label")).toMatch(
      /^Add README\.md:\d+ to a chat's context$/,
    );
    // It lists the project's chats, the one this spec started among them, by its number.
    const offered = await browser.execute(
      (menu: string) =>
        [...document.querySelectorAll(`${menu} [role="option"]`)].map((row) => ({
          session: Number(row.getAttribute("data-session") ?? -1),
          name: (row.textContent ?? "").trim(),
        })),
      PICKER,
    );
    const mine = started?.session ?? -1;
    if (!offered.some((row) => row.session === mine))
      throw new Error(
        `the chat this spec started, ${mine}, is not offered: ${offered.length} offered, the last ${JSON.stringify(offered.slice(-8))}`,
      );
    const refusedBefore = await refusals();

    await browser.execute(
      (menu: string, n: number) => {
        document
          .querySelector<HTMLElement>(`${menu} [role="option"][data-session="${n}"]`)
          ?.click();
      },
      PICKER,
      mine,
    );

    // What the core answered is said in the status line: typed into the chat unsent, or, where
    // the chat is not at its prompt, copied with the reason (`references.rs`). Either is the
    // hand, and which one depends on what the stand-in harness reports, so both pass. A refusal
    // fails at once, in the core's own words.
    let said = "";
    await browser.waitUntil(
      async () => {
        said = (await textOf(".search-summary .search-handed")) ?? "";
        return said !== "" || (await refusals()).length > refusedBefore.length;
      },
      { timeout: 20_000, interval: 250, timeoutMsg: "the status line never said what was handed" },
    );
    if (said === "")
      throw new Error(
        `nothing was handed: ${(await refusals()).slice(refusedBefore.length).join(" ")}`,
      );
    expect(said).toContain("README.md");
    await $(PICKER).waitForExist({ reverse: true, timeout: 10_000 });
  });
});
