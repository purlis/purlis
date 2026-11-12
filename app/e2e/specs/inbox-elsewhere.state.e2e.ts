// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { ASKS_A_PERMISSION, anEmptyRecord, copyFixturePlane } from "../harness.js";
import { endEveryChat, pressAndStart, showView } from "../opening.js";
import { PROJECTS, ask, inFront, openFromTheStrip } from "../switching.js";

/**
 * **Another project's ask is reached from its tab, never from the Asks in front** (#6340, #6341,
 * ADR 0121 amended 2026-11-12, decisions 1 and 4), in the built app, with an ask the reporting
 * harness really raised.
 *
 * This is the run whose harness reports through `purlis hook` (`wdio.state.conf.ts`): a chat in
 * the launch's project asks a permission through the real hook (`ASKS_A_PERMISSION`) and waits.
 * A second project of the spec's own is then opened in the same window and brought to the front.
 * Its ✋ counts its own asks alone, so none, and its Asks lists no row of another project's; the
 * launch's project's tab on the rail carries the ask instead. Pressed, that tab brings the
 * launch's project forward, whose ✋ counts it and whose own Asks lists it, and Deny pressed there
 * answers its hook: the permission is listed and held nowhere after.
 *
 * Reaching it from its notification is held by `AsksEveryProject.window.test.tsx` against a
 * mocked window: the click comes from the desktop, which no spec here can press.
 *
 * **It leaves the window as it found it**: the ask's file is removed, the chat ended, the second
 * project closed, and the launch's project is in front.
 */

const INBOX = '[role="tabpanel"][data-view="asks"]';
/** Any row or line another project's asks would be drawn as: none since #6340. */
const ELSEWHERE = `${INBOX} [data-elsewhere], ${INBOX} [role="rowgroup"][data-others]`;
/** The permission ask the harness raised, as the Asks in front draws it. */
const PERMISSION = `${INBOX} .inbox-ask[data-source="permission"]`;
const HAND = '[data-testid="title-bar"] [data-testid="needs-you-button"]';
/** The right bar's ✋: the Asks view's tab, whose name carries its count after "Asks". */
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

/** Whether anything `css` finds is drawn: a project behind keeps its views mounted, hidden. */
const drawn = (css: string) =>
  browser.execute(
    (selector: string) =>
      [...document.querySelectorAll<HTMLElement>(selector)].some(
        (one) => one.getClientRects().length > 0,
      ),
    css,
  );

/** What the Asks in front says, for a failure's message. */
const asksSay = () =>
  browser.execute(
    (inbox: string) =>
      [...document.querySelectorAll<HTMLElement>(inbox)]
        .find((one) => one.getClientRects().length > 0)
        ?.innerText.trim() ?? "(no Asks)",
    INBOX,
  );

/** What `plane`'s tab on the rail says of itself beside its name: its description, where its
 *  count is said ("1 waiting"). */
async function tabSays(plane: string): Promise<string> {
  return browser.execute(
    (strip: string, path: string) => {
      const tab = [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)].find(
        (one) => one.getAttribute("data-directory") === path,
      );
      return (tab?.getAttribute("aria-describedby") ?? "")
        .split(/\s+/)
        .map((id) => document.getElementById(id)?.textContent ?? "")
        .join(" ");
    },
    PROJECTS,
    plane,
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

describe("another project's ask, reached from its tab", function () {
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

  it("leaves the launch's project's ask out of the second project's Asks, and its tab leads to it", async () => {
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

    // The second project, in front, with no view opened first.
    await toFront(second);
    // What the ✋'s badge says: its count is in the tab's name (ADR 0121 decision 25), named by
    // `aria-labelledby` as the tab itself and then its badge.
    const handSays = () =>
      browser.execute((tab: string) => {
        const [, badge] = (
          document.querySelector(tab)?.getAttribute("aria-labelledby") ?? ""
        ).split(/\s+/);
        return badge ? (document.getElementById(badge)?.textContent ?? "") : "";
      }, ASKS_TAB);
    // The launch's project's tab says its ask; the ✋ in front, whose project asks nothing, has
    // no count (#6341).
    await browser
      .waitUntil(async () => /\b1 waiting\b/.test(await tabSays(first)), { timeout: 30_000 })
      .catch(async () => {
        throw new Error(`${first}'s tab never counted its ask; it says: ${await tabSays(first)}`);
      });
    expect(await handSays()).toBe("");
    // Its Asks lists no other project's ask, as a row or a line (#6340).
    await showView("Asks");
    await browser.waitUntil(() => drawn(INBOX), { timeout: 20_000, timeoutMsg: "no Asks drawn" });
    expect(await drawn(ELSEWHERE)).toBe(false);
    expect(await drawn(PERMISSION)).toBe(false);

    // **Reached from its tab** (#6341): pressed, the launch's project is in front, its ✋
    // counts the ask, and its own Asks lists it.
    await toFront(first);
    await browser
      .waitUntil(async () => /^1 thing waits on you$/.test(await handSays()), { timeout: 30_000 })
      .catch(async () => {
        throw new Error(`${first}'s ✋ never counted its own ask; it says: ${await handSays()}`);
      });
    await showView("Asks");
    await browser
      .waitUntil(() => drawn(PERMISSION), { timeout: 30_000 })
      .catch(async () => {
        throw new Error(
          `the Asks of ${first} never listed its own ask; it says: ${await asksSay()}`,
        );
      });

    // Deny, pressed there: a refusal is never held by the settle guard (ADR 0097).
    await browser.execute(
      (row: string) =>
        [...document.querySelectorAll<HTMLElement>(`${row} button`)]
          .find((one) => one.getClientRects().length > 0 && one.textContent?.trim() === "Deny")
          ?.click(),
      PERMISSION,
    );
    await browser
      .waitUntil(async () => !(await drawn(PERMISSION)), {
        timeout: 30_000,
        interval: 250,
      })
      .catch(async () => {
        throw new Error(`Deny never answered ${first}'s ask; the ✋ says: ${await handSays()}`);
      });
    // And the launch's project holds the permission no more. Its chat goes on after the Deny,
    // and may stop on its next turn with a question of its own: that is a new ask, not this one.
    const after = await ask<{ asks: { source: string }[] }>("asks_waiting", { plane: first });
    expect(after.asks.filter((one) => one.source === "permission")).toEqual([]);
  });
});
