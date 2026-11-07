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
import { openThroughTheTrustQuestion, showView } from "../opening.js";
import { PROJECTS, ask, inFront, openFromTheStrip } from "../switching.js";

/**
 * **Updates' doctor line: listed under Project, and Mark all read and Dismiss all act on the
 * updates alone** (#1693, f818038e; ADR 0119), in the built app against the real core and the
 * real doctor.
 *
 * The update is the doctor's, made the way `git-identity.e2e.ts` makes its finding: the run's own
 * `.gitconfig` (never the runner's) is emptied, and a project of the spec's own is opened, whose
 * preflight then finds no git identity. The finding stands twice: in the Notices view, as a
 * Notice the person answers with its Fix, and in Updates, under Project, as a line that only
 * says it happened. A finding already there the first time a project opens on a machine counts
 * as seen (#5648), so the project is opened once with the identity in place, let go, and opened
 * again without it: the finding is then new.
 *
 * - The doctor's update is listed under Project, never under Mine.
 * - Mark all read marks every update read, and the Notice still stands.
 * - Dismiss all puts every update away, and the Notice still stands; the dismissed update does
 *   not come back, its Undo past, while the doctor still says it.
 *
 * `Inbox.window.test.tsx`, `UpdatesMineProject.window.test.tsx` and `inboxUpdates.test.ts` hold
 * each rule against a mocked core.
 *
 * **Nothing left behind**: the identity is put back whatever happened, the project is let go,
 * and the launch's project is in front. What the updates' store noted is this project's, in the
 * run's own data home.
 */

const INBOX = '[role="tabpanel"][data-view="updates"]';
const NOTICES = '[role="tabpanel"][data-view="notices"]';
const UPDATES = `${INBOX} section[aria-label="Updates"]`;
const PROJECT_LIST = `${INBOX} [aria-label="Project, from the project"]`;
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

/** Whether the doctor's Notice stands in the Notices view, drawn and in view. */
async function noticeStands(): Promise<boolean> {
  return browser.execute(
    (inbox: string, cause: string) => {
      const notice = [...document.querySelectorAll<HTMLElement>(`${inbox} [data-cause]`)].find(
        (one) => one.dataset.cause === cause,
      );
      return notice !== undefined && notice.closest("[hidden]") === null;
    },
    NOTICES,
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

/** What Updates and Notices say, for a failure's message. */
async function inboxSays(): Promise<string> {
  return browser.execute(
    (views: string[]) =>
      views
        .map((view) => document.querySelector<HTMLElement>(view)?.innerText.trim() ?? "(none)")
        .join(" | "),
    [INBOX, NOTICES],
  );
}

/** Opens this spec's project again, which this machine has approved: no trust question. */
async function openAgain(): Promise<void> {
  await $(`${PROJECTS} button[aria-label="Open a project…"]`).click();
  const box = await $('[data-setting="open-by-path"] input');
  await box.waitForDisplayed({ timeout: 20_000 });
  await box.addValue(mine);
  await $('form.by-path button[type="submit"]').click();
  await openThroughTheTrustQuestion({ ifAsked: true, within: 5_000 });
  await browser.waitUntil(async () => (await inFront()) === mine, {
    timeout: 30_000,
    timeoutMsg: `${mine} never came to the front again`,
  });
}

describe("Updates' doctor line", function () {
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

  it("lists the doctor's update under Project, and the bulk acts leave the Notice alone", async () => {
    // Opened first with the identity in place, so the first-open baseline holds no finding.
    await openFromTheStrip(mine);
    await ask("close_plane", { plane: mine });
    writeFileSync(THE_RUNS_GIT_CONFIG, "");
    try {
      // The preflight is asked when a project opens: the read this spec's update comes from.
      await openAgain();
      await showView("Notices");
      await showView("Updates");
      await $(PROJECT_LIST).waitForExist({ timeout: 20_000 });
      // Mine first, where a doctor's line is never listed.
      await browser.pause(1_000);
      expect((await updates()).some((one) => one.kind === "doctor")).toBe(false);
      await $(PROJECT_LIST).click();
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
            `the doctor's finding never stood as a Notice and a Project update; they say: ${await inboxSays()}`,
          );
        });
      expect((await updates()).some((one) => !one.read)).toBe(true);

      await pressInUpdates("Mark all read");
      await browser.waitUntil(async () => (await updates()).every((one) => one.read), {
        timeout: 20_000,
        timeoutMsg: "Mark all read left an update unread",
      });
      expect((await updates()).length).toBeGreaterThan(0);
      expect(await noticeStands()).toBe(true);

      await pressInUpdates("Dismiss all");
      await browser.waitUntil(async () => (await updates()).length === 0, {
        timeout: 20_000,
        timeoutMsg: "Dismiss all left the updates listed",
      });
      expect(await noticeStands()).toBe(true);
      // The doctor still says it, and what was dismissed does not come back for that, its
      // Undo past (`UNDO_MS`, 8 s).
      await browser.pause(10_000);
      expect(await updates()).toEqual([]);
    } finally {
      putTheIdentityBack();
    }
  });
});
