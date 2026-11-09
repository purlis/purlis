// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { ASKS_A_PERMISSION, anEmptyRecord, copyFixturePlane } from "../harness.js";
import { endEveryChat, pressAndStart, showView } from "../opening.js";
import { PROJECTS, ask, inFront, openFromTheStrip } from "../switching.js";

/**
 * **The Inbox names this window's other projects that wait, with Go to project** (#1700 item 5,
 * 40784762), in the built app, with an ask the reporting harness really raised.
 *
 * This is the run whose harness reports through `purlis hook` (`wdio.state.conf.ts`): a chat in
 * the launch's project asks a permission through the real hook (`ASKS_A_PERMISSION`) and waits.
 * A second project of the spec's own is then opened in the same window and brought to the front.
 * Its ✋ counts the launch's project's ask as "+1 elsewhere" (#5999), and its Inbox, which opens
 * on All projects, lists nothing of its own and names the launch's project, with how many asks
 * wait there, under "Other projects in this window". Go to project brings that project to the front
 * with its Inbox open, where the ask is drawn whole.
 *
 * `Inbox.window.test.tsx` holds the line against a mocked window; what only the built app says
 * is that the window's own registry counts the ask in the right project.
 *
 * **It leaves the window as it found it**: the ask's file is removed, the chat ended, the second
 * project closed, and the launch's project is in front.
 */

const INBOX = '[role="tabpanel"][data-view="asks"]';
/** The group of the lines for this window's other projects: rows of the Asks list (#3123). */
const OTHERS = `${INBOX} [role="rowgroup"][data-others]`;
const HAND = '[data-testid="title-bar"] [data-testid="needs-you-button"]';
const ASK = `${INBOX} .inbox-ask[data-source="permission"]`;
/** The right bar's ✋: the Asks view's tab, whose description is its count. */
const ASKS_TAB = '[role="tablist"][aria-label="Attention"] [role="tab"][aria-label="Asks"]';

/** The second project: the fixture, renamed, with no record to put back. */
const second = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), "inbox-elsewhere");
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  anEmptyRecord(root);
  return root;
})();

/** The lines for the window's other projects, as the Inbox in front draws them; one pass. */
async function othersSaid(): Promise<{ says: string; go: string }[]> {
  return browser.execute(
    (others: string) =>
      // A project's own line, never the asks listed under it (qw359).
      [...document.querySelectorAll<HTMLElement>(`${others} > [role="row"][data-key]`)].map(
        (line) => ({
          says: line.querySelector(".ask-says")?.textContent?.trim() ?? "",
          // Its control, by the name it is heard by, never by its tag.
          go:
            line.querySelector('[aria-label^="Go to project "]')?.getAttribute("aria-label") ?? "",
        }),
      ),
    OTHERS,
  );
}

/** Brings `plane` to the front from its tab on the projects strip, pressed from the page. */
async function toFront(plane: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      if ((await inFront()) === plane) return true;
      await browser.execute(
        (strip: string, path: string) =>
          [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)]
            .find((tab) => tab.getAttribute("data-directory") === path)
            ?.click(),
        PROJECTS,
        plane,
      );
      return false;
    },
    { timeout: 20_000, interval: 250, timeoutMsg: `${plane} never came to the front` },
  );
}

describe("the Inbox of one project, about another that waits", function () {
  this.timeout(180_000);

  /** The project the launch opened, where the ask is raised. */
  let first = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
    // One app for the whole run: a chat an earlier spec left open would be in the count.
    await endEveryChat();
    writeFileSync(ASKS_A_PERMISSION, "");
  });

  after(async () => {
    rmSync(ASKS_A_PERMISSION, { force: true });
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
    await toFront(first);
    await endEveryChat();
  });

  it("names the project that waits, and Go to project brings it forward with its ask", async () => {
    // The second project is opened first: the hook waits a minute at most for an answer, and
    // the trust question is the slow part of the walk.
    await openFromTheStrip(second);
    await toFront(first);

    // The ask, in the launch's project: the harness holds its output until a line is typed,
    // then asks, and waits.
    await pressAndStart("New tab");
    const pane = await $('[data-testid="pane"]');
    await pane.click();
    await $(".xterm-helper-textarea").addValue("\n");
    await browser.waitUntil(
      async () => {
        const hand = await $(HAND);
        if (!(await hand.isExisting())) return false;
        return /^\d+ things? waits? on you$/.test((await hand.getAttribute("aria-label")) ?? "");
      },
      { timeout: 30_000, interval: 250, timeoutMsg: "the hand never counted the ask" },
    );

    // The second project, in front: its ✋ counts the launch's project's ask as elsewhere
    // (#5999, ADR 0121 decision 4), with no view opened first.
    await toFront(second);
    const handSays = () =>
      browser.execute((tab: string) => {
        const described = document.querySelector(tab)?.getAttribute("aria-describedby");
        return described ? (document.getElementById(described)?.textContent ?? "") : "";
      }, ASKS_TAB);
    await browser
      .waitUntil(async () => /, \+1 elsewhere$/.test(await handSays()), { timeout: 30_000 })
      .catch(async () => {
        throw new Error(
          `the ✋ of ${second} never counted the ask elsewhere; it says: ${await handSays()}`,
        );
      });
    // Its Inbox opens on All projects, so the other project is named without a press.
    await showView("Asks");
    await browser
      .waitUntil(async () => (await othersSaid()).length > 0, { timeout: 30_000 })
      .catch(async () => {
        throw new Error(
          `the Inbox of ${second} never named another project; it says: ${await browser.execute(
            (inbox: string) =>
              document.querySelector<HTMLElement>(inbox)?.innerText.trim() ?? "(no Inbox)",
            INBOX,
          )}`,
        );
      });
    const lines = await othersSaid();
    // One line: the launch's project, with the one ask that waits there. This project's own
    // Inbox lists no ask of its own.
    expect(lines).toHaveLength(1);
    expect(lines[0].says).toMatch(/ · 1 waiting on you$/);
    expect(lines[0].go).toMatch(/^Go to project /);
    expect(await $(ASK).isExisting()).toBe(false);

    await browser.execute(
      (others: string) =>
        document
          .querySelector<HTMLElement>(`${others} [role="row"] [aria-label^="Go to project "]`)
          ?.click(),
      OTHERS,
    );
    await browser.waitUntil(async () => (await inFront()) === first, {
      timeout: 20_000,
      timeoutMsg: "Go to project did not bring the project that waits to the front",
    });
    // Its Inbox is open on the ask, which is answered there.
    const asked = await $(ASK);
    await asked.waitForDisplayed({
      timeout: 20_000,
      timeoutMsg: "Go to project brought the project forward without its Inbox on the ask",
    });
    // And its own Inbox does not name itself among the others.
    expect(await $(OTHERS).isExisting()).toBe(false);
  });
});
