// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import {
  choose,
  closeTab,
  focusOn,
  keyWithFallback,
  tabNames,
  textOf,
  typeInto,
} from "../acting.js";
import { READY } from "../harness.js";
import { endChat, pressAndStart, showView } from "../opening.js";

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
const SCOPE = 'select[aria-label="Where to search"]';
const HITS = '[role="listbox"][aria-label="Search results"]';
const PICKER = '.search-pick [role="menu"]';
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

  before(async () => {
    wereAlreadyOpen = await tabNames();
    // A chat to hand the hit to.
    await pressAndStart("New tab");
    await browser.waitUntil(
      async () => (await $('[data-testid="pane"] .xterm-rows').getText()).includes(READY),
      { timeout: 30_000, interval: 250, timeoutMsg: "the chat never started" },
    );
    await showView("Search");
    await $(BOX).waitForExist({ timeout: 20_000 });
  });

  after(async () => {
    if (await $(BOX).isExisting()) await typeInto(BOX, "");
    for (let round = 0; round < 5; round++) {
      const mine = (await tabNames()).filter((tab) => !wereAlreadyOpen.includes(tab));
      if (mine.length === 0) break;
      // A view tab (`Files · svc`, where the driver's Enter opened the hit before the page's
      // Shift+Enter reached it) is closed; a chat this spec started is ended.
      for (const name of mine)
        if (name.includes(" · ")) await closeTab(name);
        else await endChat(`End chat ${name}`);
      await browser.pause(500);
    }
    await showView("Chats");
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
    expect(await tabNames()).not.toContain("Search");
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
    // It lists the project's chats, the one this spec started among them.
    const offered = await browser.execute(
      (menu: string) =>
        [...document.querySelectorAll(`${menu} [role="menuitem"]`)].map((row) =>
          (row.textContent ?? "").trim(),
        ),
      PICKER,
    );
    const mine = (await tabNames()).filter((tab) => !wereAlreadyOpen.includes(tab));
    const chat = offered.find((name) => mine.some((tab) => tab.includes(name)));
    if (chat === undefined)
      throw new Error(
        `none of the chats offered is the one this spec started: started ${JSON.stringify(mine)}; open before ${JSON.stringify(wereAlreadyOpen)}; ${offered.length} offered, the last ${JSON.stringify(offered.slice(-8))}`,
      );
    const refusedBefore = await refusals();

    await browser.execute(
      (menu: string, name: string) => {
        [...document.querySelectorAll<HTMLElement>(`${menu} [role="menuitem"]`)]
          .find((row) => (row.textContent ?? "").trim() === name)
          ?.click();
      },
      PICKER,
      chat,
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
