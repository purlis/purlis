// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { ASKS_A_PERMISSION, anEmptyRecord, copyFixturePlane } from "../harness.js";
import { endEveryChat, pressAndStart, showView } from "../opening.js";
import { PROJECTS, ask, inFront, openFromTheStrip } from "../switching.js";

/**
 * **Another project's ask is answered in place from Asks** (#1921, ADR 0121 decision 4; first
 * #1700 item 5), in the built app, with an ask the reporting harness really raised.
 *
 * This is the run whose harness reports through `purlis hook` (`wdio.state.conf.ts`): a chat in
 * the launch's project asks a permission through the real hook (`ASKS_A_PERMISSION`) and waits.
 * A second project of the spec's own is then opened in the same window and brought to the front.
 * Its ✋ counts its own asks alone, so none (#6341, ADR 0121 amended 2026-11-12, decision 1), and
 * the launch's project's tab on the rail carries the ask instead; pressed, that tab brings the
 * launch's project forward, whose ✋ counts it. Back in the second project, its Asks, which opens on All
 * projects, lists that ask as a row of its own, with its options on the row and no line for the
 * project. Deny, pressed there, is answered in the launch's project: its hook is
 * answered, and the permission is listed and held nowhere after.
 *
 * `AsksInPlace.window.test.tsx` holds the rows against a mocked window; what only the built app
 * says is that the window's registry lists the ask under the right project and that an answer
 * pressed in another project's Asks reaches the hook that holds it.
 *
 * **It leaves the window as it found it**: the ask's file is removed, the chat ended, the second
 * project closed, and the launch's project is in front.
 */

const INBOX = '[role="tabpanel"][data-view="asks"]';
/** The lines for projects whose asks are not read yet: none once they are (#1921). */
const OTHERS = `${INBOX} [role="rowgroup"][data-others]`;
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

/** The groups of `plane`'s chats as another project's Asks lists them, and each row's answers,
 *  by the names they are heard by; one pass. */
async function theirRows(plane: string): Promise<{ says: string; answers: string[] }[]> {
  return browser.execute(
    (inbox: string, of: string) =>
      [...document.querySelectorAll<HTMLElement>(`${inbox} [data-elsewhere^="${of}#"]`)].flatMap(
        (group) =>
          [...group.querySelectorAll<HTMLElement>('.inbox-ask[role="row"]')].map((row) => ({
            says: row.querySelector(".ask-says")?.textContent?.trim() ?? "",
            answers: [...row.querySelectorAll<HTMLElement>('[role="button"], button')].map(
              (one) => one.textContent?.trim() ?? "",
            ),
          })),
      ),
    INBOX,
    plane,
  );
}

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

describe("another project's ask, answered in place from Asks", function () {
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

  it("lists the launch's project's ask in the second project's Asks, and Deny there answers it", async () => {
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
    // **Reached from its tab** (#6341): pressed, the launch's project is in front, and its ✋
    // counts the ask; then back to the second project, whose Asks answers it in place.
    await toFront(first);
    await browser
      .waitUntil(async () => /^1 thing waits on you$/.test(await handSays()), { timeout: 30_000 })
      .catch(async () => {
        throw new Error(`${first}'s ✋ never counted its own ask; it says: ${await handSays()}`);
      });
    await toFront(second);
    // Its Asks opens on All projects, so the other project's ask is listed without a press.
    await showView("Asks");
    await browser
      .waitUntil(async () => (await theirRows(first)).length > 0, { timeout: 30_000 })
      .catch(async () => {
        throw new Error(
          `the Asks of ${second} never listed ${first}'s ask; it says: ${await browser.execute(
            (inbox: string) =>
              document.querySelector<HTMLElement>(inbox)?.innerText.trim() ?? "(no Inbox)",
            INBOX,
          )}`,
        );
      });
    const rows = await theirRows(first);
    // One row: the launch's project's permission, with its answers on the row, and no line for
    // the project as well.
    expect(rows).toHaveLength(1);
    expect(rows[0].answers).toContain("Deny");
    expect(await $(OTHERS).isExisting()).toBe(false);

    // Deny, pressed here: a refusal is never held by the settle guard (ADR 0097).
    await browser.execute(
      (inbox: string, of: string) =>
        [
          ...document.querySelectorAll<HTMLElement>(
            `${inbox} [data-elsewhere^="${of}#"] .inbox-ask[role="row"] button`,
          ),
        ]
          .find((one) => one.textContent?.trim() === "Deny")
          ?.click(),
      INBOX,
      first,
    );
    // Answered in the launch's project: its permission's row goes from here.
    const permissionRow = `${INBOX} [data-elsewhere^="${first}#"] .inbox-ask[data-source="permission"]`;
    await browser
      .waitUntil(async () => !(await $(permissionRow).isExisting()), {
        timeout: 30_000,
        interval: 250,
      })
      .catch(async () => {
        throw new Error(
          `Deny from ${second}'s Asks never answered ${first}'s ask; the ✋ says: ${await handSays()}`,
        );
      });
    // And the launch's project holds the permission no more. Its chat goes on after the Deny,
    // and may stop on its next turn with a question of its own: that is a new ask, not this one.
    const after = await ask<{ asks: { source: string }[] }>("asks_waiting", { plane: first });
    expect(after.asks.filter((one) => one.source === "permission")).toEqual([]);
  });
});
