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
import { PROJECTS, ask, inFront, openFromTheStrip } from "../switching.js";

/**
 * **Mark all read and Dismiss all act on the Inbox's updates alone, with a real ask waiting**
 * (#1693, D-1693-1), in the built app against the real core, the real doctor and the real hook.
 *
 * `inbox-updates.e2e.ts` holds the bulk acts against the Notices only: the base run's harness
 * raises no ask. This is the run whose harness asks through `purlis hook` (`wdio.state.conf.ts`),
 * so both sources stand in one project's Inbox at once:
 *
 * - **the update** is the doctor's git-identity finding, the one update the run can make
 *   deterministically (a finished task's update needs a core that dispatched one): the run's own
 *   `.gitconfig` is emptied and a project of the spec's own is opened, whose preflight finds no
 *   identity. The identity is put back at once: a doctor's update is not a live kind
 *   (`inboxUpdates.ts`), so it stays listed after the finding clears, and the chat below starts
 *   with the run's identity as every other chat does;
 * - **the ask** is the reporting harness's permission ask (`ASKS_A_PERMISSION`). The project
 *   declares that harness as its own profile, the piece D-1693-1 named as missing: the state
 *   run declares it in the launch's project only.
 *
 * After Mark all read, and again after Dismiss all, the ask is still listed with its Allow, and
 * the title bar's hand still counts it. Then Allow is pressed, and the chat carrying on proves
 * the bulk acts answered nothing for it.
 *
 * **Nothing left behind**, each step tried whatever became of the one before it: the ask's file
 * is removed, the identity put back, the chat ended by its number, the project closed, the
 * launch's project in front, and the left side on Chats.
 */

const INBOX = '[role="tabpanel"][data-view="inbox"]';
const UPDATES = `${INBOX} section[aria-label="Updates"]`;
const ASK = `${INBOX} .inbox-ask[data-source="permission"]`;
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

/** The permission ask as the Inbox draws it: what it says, and whether its Allow can be pressed. */
async function theAsk(): Promise<{ says: string; allow: boolean } | null> {
  return browser.execute((selector: string) => {
    const row = document.querySelector<HTMLElement>(selector);
    if (row === null) return null;
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

describe("the Inbox's bulk acts on updates, with an ask waiting", function () {
  this.timeout(240_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";
  /** The chat that asks, by the number its start answered. */
  let asking: Started | undefined;

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];

    // The update: the preflight is asked when a project opens, and finds no identity.
    writeFileSync(THE_RUNS_GIT_CONFIG, "");
    try {
      await openFromTheStrip(mine);
      await showView("Inbox");
      await browser
        .waitUntil(
          async () =>
            (await updates()).some(
              (one) => one.kind === "doctor" && one.says.includes("git identity"),
            ),
          { timeout: 60_000, interval: 500 },
        )
        .catch(async () => {
          throw new Error(
            `the doctor's finding never stood as an update; the Inbox says: ${await inboxSays()}`,
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
    await showView("Inbox");
    await browser
      .waitUntil(async () => (await theAsk())?.says.includes(ASKED_COMMAND) ?? false, {
        timeout: 30_000,
        interval: 250,
      })
      .catch(async () => {
        throw new Error(
          `chat ${session}'s ask never stood in the Inbox; it says: ${await inboxSays()}`,
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

  it("marks every update read and puts them away, and the ask still waits to be answered", async () => {
    await showView("Inbox");
    let counted = "";
    await browser.waitUntil(
      async () => {
        counted = await handSays();
        return WAITS.test(counted);
      },
      { timeout: 20_000, interval: 250, timeoutMsg: "the hand never counted the ask" },
    );
    const listed = await updates();
    expect(listed.length).toBeGreaterThan(0);
    expect(listed.some((one) => !one.read)).toBe(true);

    await pressInUpdates("Mark all read");
    await browser.waitUntil(async () => (await updates()).every((one) => one.read), {
      timeout: 20_000,
      timeoutMsg: "Mark all read left an update unread",
    });
    expect((await updates()).length).toBeGreaterThan(0);
    await theAskStillWaits("Mark all read", counted);

    await pressInUpdates("Dismiss all");
    await browser.waitUntil(async () => !(await $(UPDATES).isExisting()), {
      timeout: 20_000,
      timeoutMsg: "Dismiss all left the updates listed",
    });
    await theAskStillWaits("Dismiss all", counted);

    // Answered now, and only now: the chat carrying on is the proof that neither bulk act
    // answered it. An Allow waits for its row to settle (`SETTLE_MS`, 1.5 s): the person reads
    // the ask before allowing it, so a press too soon after the updates went is pressed again.
    const session = asking?.session ?? -1;
    await browser
      .waitUntil(
        async () => {
          if ((await paneSays(session)).includes(INBOX_ALLOWED)) return true;
          await browser.pause(2_000);
          await browser.execute((selector: string) => {
            [...(document.querySelector(selector)?.querySelectorAll<HTMLElement>("button") ?? [])]
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
      timeoutMsg: "the answered ask stayed in the Inbox",
    });
  });
});
