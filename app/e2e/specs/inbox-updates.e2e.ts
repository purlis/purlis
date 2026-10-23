// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { realpathSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  anEmptyRecord,
  copyFixturePlane,
  THE_RUNS_GIT_CONFIG,
  THE_RUNS_GIT_CONFIG_TEXT,
} from "../harness.js";
import { showView } from "../opening.js";
import { PROJECTS, ask, inFront, openFromTheStrip } from "../switching.js";

/**
 * **The Inbox's updates: listed after what waits, and Mark all read and Dismiss all act on them
 * alone** (#1693, f818038e), in the built app against the real core and the real doctor.
 *
 * The update is the doctor's, made the way `git-identity.e2e.ts` makes its finding: the run's own
 * `.gitconfig` (never the runner's) is emptied, and a project of the spec's own is opened, whose
 * preflight then finds no git identity. The finding stands twice in that project's Inbox: as a
 * Notice, which the person answers with its Fix, and as an update, which only says it happened.
 *
 * - The updates are listed after the Notices.
 * - Mark all read marks every update read, and the Notice still stands.
 * - Dismiss all puts every update away, and the Notice still stands; the dismissed update does
 *   not come back while the doctor still says it.
 *
 * `Inbox.window.test.tsx` and `inboxUpdates.test.ts` hold each rule against a mocked core,
 * asks included; this run's harness raises none, so what is held here is that the bulk acts
 * leave the Notices alone.
 *
 * **Nothing left behind**: the identity is put back whatever happened, the project is let go,
 * and the launch's project is in front. What the updates' store noted is this project's, in the
 * run's own data home.
 */

const INBOX = '[role="tabpanel"][data-view="inbox"]';
const UPDATES = `${INBOX} section[aria-label="Updates"]`;
const CAUSE = "doctor-finding:git-identity";

/** This spec's own project: the fixture, renamed, with nothing to put back on open. */
const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), "inbox-updates");
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  anEmptyRecord(root);
  return root;
})();

/** The run's identity, as every other spec has it. */
const putTheIdentityBack = () => writeFileSync(THE_RUNS_GIT_CONFIG, THE_RUNS_GIT_CONFIG_TEXT);

type Update = { kind: string; read: boolean; says: string };

/** The Inbox's updates, as it draws them; one pass. */
async function updates(): Promise<Update[]> {
  return browser.execute(
    (section: string) =>
      [...document.querySelectorAll<HTMLElement>(`${section} li.inbox-update`)].map((row) => ({
        kind: row.dataset.kind ?? "",
        read: row.dataset.read !== undefined,
        says: row.innerText.trim(),
      })),
    UPDATES,
  );
}

/** Whether the doctor's Notice stands in the Inbox, drawn and in view. */
async function noticeStands(): Promise<boolean> {
  return browser.execute(
    (inbox: string, cause: string) => {
      const notice = [...document.querySelectorAll<HTMLElement>(`${inbox} [data-cause]`)].find(
        (one) => one.dataset.cause === cause,
      );
      return notice !== undefined && notice.closest("[hidden]") === null;
    },
    INBOX,
    CAUSE,
  );
}

/** Presses the updates' own button that says `words`, from the page. */
async function pressInUpdates(words: string): Promise<void> {
  const pressed = await browser.execute(
    (section: string, said: string) => {
      const button = [...document.querySelectorAll<HTMLButtonElement>(`${section} button`)].find(
        (one) =>
          one.textContent?.trim() === said &&
          !one.disabled &&
          one.getAttribute("aria-disabled") !== "true",
      );
      button?.click();
      return button !== undefined;
    },
    UPDATES,
    words,
  );
  if (!pressed) throw new Error(`the updates offer no ${words} to press`);
}

/** What the Inbox says, for a failure's message. */
async function inboxSays(): Promise<string> {
  return browser.execute(
    (inbox: string) => document.querySelector<HTMLElement>(inbox)?.innerText.trim() ?? "(none)",
    INBOX,
  );
}

describe("the Inbox's updates", function () {
  this.timeout(180_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
  });

  after(async () => {
    putTheIdentityBack();
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
    await browser.waitUntil(
      async () => {
        if ((await inFront()) === first) return true;
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
  });

  it("lists the doctor's update after its Notice, and the bulk acts leave the Notice alone", async () => {
    writeFileSync(THE_RUNS_GIT_CONFIG, "");
    try {
      // The preflight is asked when a project opens: the read this spec's update comes from.
      await openFromTheStrip(mine);
      await showView("Inbox");
      await browser
        .waitUntil(
          async () =>
            (await noticeStands()) &&
            (await updates()).some(
              (one) => one.kind === "doctor" && one.says.includes("git identity"),
            ),
          { timeout: 60_000, interval: 500 },
        )
        .catch(async () => {
          throw new Error(
            `the doctor's finding never stood as a Notice and an update; the Inbox says: ${await inboxSays()}`,
          );
        });

      // After the Notices, in the Inbox's own order.
      const after = await browser.execute(
        (inbox: string, section: string) => {
          const notices = document.querySelector(`${inbox} section.inbox-notices`);
          const listed = document.querySelector(section);
          return (
            notices !== null &&
            listed !== null &&
            (notices.compareDocumentPosition(listed) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0
          );
        },
        INBOX,
        UPDATES,
      );
      expect(after).toBe(true);
      expect((await updates()).some((one) => !one.read)).toBe(true);

      await pressInUpdates("Mark all read");
      await browser.waitUntil(async () => (await updates()).every((one) => one.read), {
        timeout: 20_000,
        timeoutMsg: "Mark all read left an update unread",
      });
      expect((await updates()).length).toBeGreaterThan(0);
      expect(await noticeStands()).toBe(true);

      await pressInUpdates("Dismiss all");
      await browser.waitUntil(async () => !(await $(UPDATES).isExisting()), {
        timeout: 20_000,
        timeoutMsg: "Dismiss all left the updates listed",
      });
      expect(await noticeStands()).toBe(true);
      // The doctor still says it, and what was dismissed does not come back for that.
      await browser.pause(3_000);
      expect(await updates()).toEqual([]);
    } finally {
      putTheIdentityBack();
    }
  });
});
