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
import {
  PROJECTS,
  ask,
  inFront,
  asksTheDoctorAgain,
  keepTheNotices,
  openedWithTheDoctorHeard,
} from "../switching.js";

/**
 * **The doctor's findings stand as one Notice line, never as an update, and Dismiss all puts it
 * away until a finding changes** (#5645, #5650; ADR 0119), in the built app against the real
 * core and the real doctor.
 *
 * The finding is made the way `git-identity.e2e.ts` makes it: the run's own `.gitconfig` (never
 * the runner's) is emptied, and the doctor of a project of the spec's own then finds no git
 * identity. A finding already there the first time a project opens on a machine counts as
 * seen (#5648), so the project is opened with the identity in place and its doctor heard, and
 * the doctor is asked again, from its status item, once the identity is gone: the finding is
 * then new.
 *
 * - The Notices view lists it as the doctor's one line, "Doctor: N findings".
 * - Updates lists no line for it, under Mine or Project: a doctor finding is a Notice only.
 * - Dismiss all puts the line away, and it does not come back, its Undo past, while the doctor
 *   still says the same.
 *
 * `DoctorNotice.window.test.tsx`, `DismissAll.window.test.tsx` and `inboxUpdates.test.ts` hold
 * each rule against a mocked core.
 *
 * **Nothing left behind**: the identity is put back whatever happened, the project is let go,
 * and the launch's project is in front. What the stores noted is this project's, in the run's
 * own data home.
 */

const UPDATES_VIEW = '[role="tabpanel"][data-view="updates"]';
const NOTICES_VIEW = '[role="tabpanel"][data-view="notices"]';
const NOTICES = `${NOTICES_VIEW} section[aria-label="Notices"]`;
const PROJECT_LIST = `${UPDATES_VIEW} [aria-label="Project, from the project"]`;
/** The doctor's one line (`DOCTOR_LINE`, `Doctor.tsx`). */
const DOCTOR_LINE = `${NOTICES_VIEW} [data-cause="doctor-finding:*"]`;

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

/** The kinds of the lines Updates lists now, under the list picked; one pass. */
async function updateKinds(): Promise<string[]> {
  return browser.execute(
    (view: string) =>
      [...document.querySelectorAll<HTMLElement>(`${view} li.inbox-update`)].map(
        (row) => row.dataset.kind ?? "",
      ),
    UPDATES_VIEW,
  );
}

/** What the doctor's line says, or `null` where it is not drawn in view. */
async function doctorLine(): Promise<string | null> {
  return browser.execute((line: string) => {
    const found = document.querySelector<HTMLElement>(line);
    return found === null || found.closest("[hidden]") !== null ? null : found.innerText.trim();
  }, DOCTOR_LINE);
}

/** Presses the Notices' own button that says `words`, from the page. */
async function pressInNotices(words: string): Promise<void> {
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
    NOTICES,
    words,
  );
  if (!pressed) throw new Error(`the Notices offer no ${words} to press`);
}

/** What Notices and Updates say, for a failure's message. */
async function viewsSay(): Promise<string> {
  return browser.execute(
    (views: string[]) =>
      views
        .map((view) => document.querySelector<HTMLElement>(view)?.innerText.trim() ?? "(none)")
        .join(" | "),
    [NOTICES_VIEW, UPDATES_VIEW],
  );
}

describe("the doctor's Notice line", function () {
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

  it("stands as one Notice line and no update, and Dismiss all puts it away", async () => {
    // Opened first with the identity in place, so the first-open baseline holds no finding.
    await openedWithTheDoctorHeard(mine);
    writeFileSync(THE_RUNS_GIT_CONFIG, "");
    try {
      // The doctor asked again, from its status item: the read this spec's finding comes from.
      const found = await asksTheDoctorAgain();
      await showView("Notices");
      await browser
        .waitUntil(async () => /Doctor: \d+ findings?/.test((await doctorLine()) ?? ""), {
          timeout: 60_000,
          interval: 500,
        })
        .catch(async () => {
          throw new Error(
            `the doctor's line never stood; the doctor found: ${found}; the views say: ${await viewsSay()}; kept: ${await keepTheNotices("inbox-updates")}`,
          );
        });

      // No update for it, under either list.
      await showView("Updates");
      await browser.pause(1_000);
      expect(await updateKinds()).not.toContain("doctor");
      await $(PROJECT_LIST).click();
      await browser.pause(1_000);
      expect(await updateKinds()).not.toContain("doctor");

      await pressInNotices("Dismiss all");
      await browser.waitUntil(async () => (await doctorLine()) === null, {
        timeout: 20_000,
        timeoutMsg: "Dismiss all left the doctor's line listed",
      });
      // The doctor still says it, and what was dismissed does not come back for that, its
      // Undo past (`UNDO_MS`, 8 s).
      await browser.pause(10_000);
      expect(await doctorLine()).toBeNull();
    } finally {
      putTheIdentityBack();
    }
  });
});
