// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  ASKED_COMMAND,
  ASKS_A_PERMISSION,
  INBOX_ALLOWED,
  THE_RUNS_GIT_CONFIG,
  THE_RUNS_GIT_CONFIG_TEXT,
  anEmptyRecord,
  built,
  copyFixturePlane,
  declareAProfile,
  writeReportingShell,
} from "../harness.js";
import { closeProject, showView } from "../opening.js";
import { endChatById, startAChat, type Started } from "../started.js";
import {
  PROJECTS,
  ask,
  inFront,
  asksTheDoctorAgain,
  openedWithTheDoctorHeard,
} from "../switching.js";

/**
 * **Dismiss all on the Notices answers nothing, with a real ask waiting in Asks** (#1693,
 * D-1693-1, #5650; ADR 0119), in the built app against the real core, the real doctor and the
 * real hook.
 *
 * `inbox-updates.e2e.ts` holds the doctor's Notice line and its Dismiss all where no ask
 * stands: the base run's harness raises none. This is the run whose harness asks through
 * `purlis hook` (`wdio.state.conf.ts`), so both stand in one project at once:
 *
 * - **the Notice** is the doctor's one line (#5645), from its git-identity finding, the one the
 *   run can make deterministically: the run's own `.gitconfig` is emptied and a project of the
 *   spec's own is checked again by the doctor, which finds no identity. Opened with the identity
 *   in place and its doctor heard first, so the first-open baseline (#5648) does not count the
 *   finding as seen.
 *   The identity is put back at once: the doctor is not asked again, so the line stands until
 *   it is put away, and the chat below starts with the run's identity as every other does;
 * - **the ask** is the reporting harness's permission ask (`ASKS_A_PERMISSION`). The project
 *   declares that harness as its own profile, the piece D-1693-1 named as missing: the state
 *   run declares it in the launch's project only.
 *
 * After Dismiss all, the ask is still listed with its Allow, and the title bar's hand still
 * counts it. Then Allow is pressed, and the chat carrying on proves the bulk act answered nothing
 * for it.
 *
 * **Nothing left behind**, each step tried whatever became of the one before it: the ask's file
 * is removed, the identity put back, the chat ended by its number, the project closed, the
 * launch's project in front, and the left side on Chats.
 */

const NOTICES_VIEW = '[role="tabpanel"][data-view="notices"]';
const NOTICES = `${NOTICES_VIEW} section[aria-label="Notices"]`;
/** The doctor's one line (`DOCTOR_LINE`, `Doctor.tsx`). */
const DOCTOR_LINE = `${NOTICES_VIEW} [data-cause="doctor-finding:*"]`;
const ASKS = '[role="tabpanel"][data-view="asks"]';
const ASK = `${ASKS} .inbox-ask[data-source="permission"]`;
const HAND = '[data-testid="title-bar"] [data-testid="needs-you-button"]';
const WAITS = /^\d+ (thing waits on|things wait on|chat needs|chats need) you$/;

/** This spec's own project: the fixture, renamed, with the reporting harness as its profile. */
const NAME = "inbox-updates-state";
const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), NAME);
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  anEmptyRecord(root);
  // `codex`, as the state run declares it: the kind this stand-in behaves like.
  declareAProfile(root, writeReportingShell(built("fake-harness"), built("charter")), "codex");
  return root;
})();

/** The run's identity, as every other spec has it. */
const putTheIdentityBack = () => writeFileSync(THE_RUNS_GIT_CONFIG, THE_RUNS_GIT_CONFIG_TEXT);

/** Whether the doctor's line is drawn in the Notices view. */
async function doctorLineStands(): Promise<boolean> {
  return browser.execute((line: string) => document.querySelector(line) !== null, DOCTOR_LINE);
}

/** The permission ask as Asks draws it: what it says, and whether its Allow can be pressed. Its
 *  form stands behind its Answer link (#5651), which this opens where it is closed. */
async function theAsk(): Promise<{ says: string; allow: boolean } | null> {
  return browser.execute((selector: string) => {
    const row = document.querySelector<HTMLElement>(selector);
    if (row === null) return null;
    const answer = [...row.querySelectorAll<HTMLButtonElement>("button")].find(
      (one) => one.textContent?.trim() === "Answer" && one.getAttribute("aria-expanded") !== "true",
    );
    answer?.click();
    const allow = [...row.querySelectorAll<HTMLButtonElement>("button")].find(
      (one) => one.textContent?.trim() === "Allow",
    );
    return {
      says: row.innerText.trim(),
      allow:
        allow !== undefined && !allow.disabled && allow.getAttribute("aria-disabled") !== "true",
    };
  }, ASK);
}

/** What the title bar's hand says, or nothing where it is not drawn. */
async function handSays(): Promise<string> {
  return browser.execute(
    (hand: string) => document.querySelector(hand)?.getAttribute("aria-label") ?? "",
    HAND,
  );
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

/** What the Inbox says, for a failure's message. */
async function inboxSays(): Promise<string> {
  return browser.execute(
    (views: string[]) =>
      views
        .map((view) => document.querySelector<HTMLElement>(view)?.innerText.trim() ?? "(none)")
        .join(" | "),
    [ASKS, NOTICES_VIEW],
  );
}

/** What chat `session`'s pane shows. */
async function paneSays(session: number): Promise<string> {
  return browser.execute(
    (n: number) =>
      document.querySelector(`[data-testid="pane"][data-session="${n}"] .xterm-rows`)
        ?.textContent ?? "",
    session,
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

/** Holds that the ask still waits, Allow and all, and that the hand still counts it. */
async function theAskStillWaits(after: string, counted: string): Promise<void> {
  let seen: Awaited<ReturnType<typeof theAsk>> = null;
  await browser
    .waitUntil(
      async () => {
        seen = await theAsk();
        return seen !== null && seen.allow;
      },
      { timeout: 10_000, interval: 250 },
    )
    .catch(async () => {
      throw new Error(
        `after ${after} the ask was ${JSON.stringify(seen)}; the Inbox says: ${await inboxSays()}`,
      );
    });
  expect(await handSays()).toBe(counted);
}

describe("the Notices' Dismiss all, with an ask waiting in Asks", function () {
  this.timeout(240_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";
  /** The chat that asks, by the number its start answered. */
  let asking: Started | undefined;

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];

    // Opened first with the identity in place, so the first-open baseline holds no finding.
    await openedWithTheDoctorHeard(mine);
    // The Notice: the doctor, asked again, finds no identity.
    writeFileSync(THE_RUNS_GIT_CONFIG, "");
    try {
      const found = await asksTheDoctorAgain();
      await showView("Notices");
      await browser
        .waitUntil(doctorLineStands, { timeout: 60_000, interval: 500 })
        .catch(async () => {
          throw new Error(
            `the doctor's finding never stood as its Notice line; the doctor found: ${found}; the views say: ${await inboxSays()}`,
          );
        });
    } finally {
      putTheIdentityBack();
    }

    // The ask: the harness holds its output until a line is typed, then asks, and waits. The
    // hook waits about a minute for an answer, so everything below is quick.
    writeFileSync(ASKS_A_PERMISSION, "");
    asking = await startAChat();
    const session = asking.session;
    await browser.execute((n: number) => {
      document.querySelector<HTMLElement>(`[data-testid="pane"][data-session="${n}"]`)?.click();
    }, session);
    await $(`[data-testid="pane"][data-session="${session}"] .xterm-helper-textarea`).addValue(
      "\n",
    );
    await showView("Asks");
    await browser
      .waitUntil(async () => (await theAsk())?.says.includes(ASKED_COMMAND) ?? false, {
        timeout: 30_000,
        interval: 250,
      })
      .catch(async () => {
        throw new Error(
          `chat ${session}'s ask never stood in Asks; they say: ${await inboxSays()}`,
        );
      });
  });

  // Each step is tried whatever became of the one before it: one app process serves the run.
  after(async () => {
    const tried = async (what: string, step: () => Promise<unknown> | unknown) => {
      try {
        await step();
      } catch (trouble) {
        console.warn(`inbox-updates.state: ${what} failed`, trouble);
      }
    };
    await tried("removing the ask's file", () => rmSync(ASKS_A_PERMISSION, { force: true }));
    await tried("putting the identity back", putTheIdentityBack);
    if (asking !== undefined) {
      const { session, plane } = asking;
      await tried("ending the chat", () => endChatById(session, plane));
    }
    await tried("closing the project from its tab", () =>
      closeProject(`${PROJECTS} [role="tab"][aria-label="${NAME}"]`),
    );
    await tried("closing the project", async () => {
      if ((await ask<string[]>("open_planes")).includes(mine))
        await ask("close_plane", { plane: mine });
    });
    await tried("bringing the launch's project back", () => toFront(first));
    await tried("showing Chats on the left", () => showView("Chats"));
  });

  it("puts the Notices away, and the ask still waits to be answered", async () => {
    await showView("Notices");
    let counted = "";
    await browser.waitUntil(
      async () => {
        counted = await handSays();
        return WAITS.test(counted);
      },
      { timeout: 20_000, interval: 250, timeoutMsg: "the hand never counted the ask" },
    );
    expect(await doctorLineStands()).toBe(true);

    await pressInNotices("Dismiss all");
    await browser.waitUntil(async () => !(await doctorLineStands()), {
      timeout: 20_000,
      timeoutMsg: "Dismiss all left the doctor's line listed",
    });
    await theAskStillWaits("Dismiss all", counted);

    await showView("Asks");
    // Answered now, and only now: the chat carrying on is the proof that the bulk act
    // answered it. An Allow waits for its row to settle (`SETTLE_MS`, 1.5 s): the person reads
    // the ask before allowing it, so a press too soon after the updates went is pressed again.
    const session = asking?.session ?? -1;
    await browser
      .waitUntil(
        async () => {
          if ((await paneSays(session)).includes(INBOX_ALLOWED)) return true;
          await browser.pause(2_000);
          await browser.execute((selector: string) => {
            const buttons = () => [
              ...(document.querySelector(selector)?.querySelectorAll<HTMLElement>("button") ?? []),
            ];
            // Its form stands behind its Answer link (#5651).
            buttons()
              .find(
                (one) =>
                  one.textContent?.trim() === "Answer" &&
                  one.getAttribute("aria-expanded") !== "true",
              )
              ?.click();
            buttons()
              .find((one) => one.textContent?.trim() === "Allow")
              ?.click();
          }, ASK);
          return false;
        },
        { timeout: 30_000, interval: 1_000 },
      )
      .catch(async () => {
        throw new Error(
          `chat ${session} never carried on after Allow; the Inbox says: ${await inboxSays()}`,
        );
      });
    await browser.waitUntil(async () => (await theAsk()) === null, {
      timeout: 20_000,
      timeoutMsg: "the answered ask stayed in Asks",
    });
  });
});
