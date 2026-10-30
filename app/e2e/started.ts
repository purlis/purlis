// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { pressAndStart, showView } from "./opening.js";
import { projectInFront, runningIn } from "./running.js";

/**
 * **A chat a spec started, known by the number the core gave it** (#1796): never by its name.
 *
 * Two chats may share a name, a strip draws a name cut short or hides a tab past its edge, and
 * the tab in front is the OLD chat until the new one is up: a spec that finds "its" chat by a
 * name, a strip diff or the first tab can press another spec's chat. So a start answers with the
 * session the core's `running_sessions` did not list before it, and everything a spec does to
 * its chat after that goes by that number: the Chats list's row carries it (`data-session`,
 * ChatsSection.tsx) and the pane in front carries it, so the tab in front is the chat's own once
 * its pane is.
 *
 * Written for the engine CI drives it on (reach.ts says why): every press is the element's own
 * `click()` from the page, and every wait says what it was waiting for.
 */

/** A chat a spec started: the project it is in, its number there, and its tab's name then. */
export type Started = { plane: string; session: number; name: string };

const TABS = '[data-strip="Tabs"]';

/** The tab in front on the strip: the chat's own once its pane is in front. */
export const SELECTED_TAB = `${TABS} [role="tab"][aria-selected="true"]`;

/**
 * **Starts a chat by pressing `press` and answering the picker, and answers with its number.**
 * The number is the one session `running_sessions` lists after the start and did not before:
 * the core's answer, not a tab's words. It is answered once that chat's pane is in front.
 */
export async function startAChat(press = "New tab"): Promise<Started> {
  const plane = await projectInFront();
  const before = new Set(await runningIn(plane));
  await pressAndStart(press);
  let fresh: number[] = [];
  await browser.waitUntil(
    async () => {
      fresh = (await runningIn(plane)).filter((one) => !before.has(one));
      return fresh.length > 0;
    },
    { timeout: 30_000, interval: 200, timeoutMsg: `${press} started no chat in ${plane}` },
  );
  if (fresh.length !== 1)
    throw new Error(`${press} started ${fresh.length} chats (${fresh.join(", ")}), not one`);
  const [session] = fresh;
  await untilInFront(session);
  const name = await browser.execute(
    (selected: string) =>
      document.querySelector(selected)?.querySelector(".tab-name")?.textContent ?? "",
    SELECTED_TAB,
  );
  return { plane, session, name };
}

/**
 * **The project's default profile, approved as the operator approves it** (#3562): a chat
 * started through the picker, which asks once ("Approve and start"), and then ended. For a spec
 * whose chats the core starts without the picker (the Skill studio's Try it and Ask a chat to
 * draft a skill), which the core refuses on a profile nobody has approved: run first in a fresh
 * app, such a spec started nothing, since the approval was an earlier spec's to pay.
 */
export async function theDefaultProfileApproved(): Promise<void> {
  const started = await startAChat();
  await endChatById(started.session, started.plane);
}

/** The chat whose pane is drawn in front, by its number, or -1 where none is: read at once. */
export async function paneInFront(): Promise<number> {
  return browser.execute(() => {
    const pane = [...document.querySelectorAll<HTMLElement>('[data-testid="pane"]')].find(
      (one) => one.getBoundingClientRect().width > 0,
    );
    return Number(pane?.getAttribute("data-session") ?? -1);
  });
}

/**
 * Whether chat `session`'s pane is drawn, read at once. In a split tab two panes are drawn side
 * by side, and the chat may be either: so not only the first one drawn ({@link paneInFront}).
 */
async function paneShown(session: number): Promise<boolean> {
  return browser.execute(
    (n: number) =>
      [...document.querySelectorAll<HTMLElement>(`[data-testid="pane"][data-session="${n}"]`)].some(
        (one) => one.getBoundingClientRect().width > 0,
      ),
    session,
  );
}

/** Waits until chat `session`'s pane is drawn, alone or beside another in a split. */
async function untilInFront(session: number, timeout = 20_000): Promise<void> {
  await browser.waitUntil(async () => paneShown(session), {
    timeout,
    interval: 100,
    timeoutMsg: `chat ${session}'s pane was never drawn`,
  });
}

/**
 * **Brings chat `session` forward**, by its row in the Chats list: the row carries the number,
 * and a press on it shows its chat. Where the list does not draw the row (a scope or a filter a
 * spec left picked), each chat tab on the strip is brought forward in turn until the pane in
 * front is that chat's: never a tab found by its name.
 */
export async function bringChatForward(session: number): Promise<void> {
  if (await paneShown(session)) return;
  await showView("Chats");
  const pressed = await browser
    .waitUntil(
      async () =>
        browser.execute((n: number) => {
          const row = document.querySelector<HTMLElement>(
            `[role="treeitem"].chat[data-session="${n}"]`,
          );
          row?.click();
          return row !== null;
        }, session),
      { timeout: 5_000, interval: 200 },
    )
    .then(() => true)
    .catch(() => false);
  if (pressed) {
    await untilInFront(session);
    return;
  }
  const count = await browser.execute(
    (strip: string) => document.querySelectorAll(`${strip} [role="tab"]`).length,
    TABS,
  );
  for (let at = 0; at < count; at++) {
    await browser.execute(
      (strip: string, index: number) => {
        document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)[index]?.click();
      },
      TABS,
      at,
    );
    const came = await browser
      .waitUntil(async () => paneShown(session), { timeout: 2_000, interval: 100 })
      .then(() => true)
      .catch(() => false);
    if (came) return;
  }
  throw new Error(`chat ${session} is neither a row in the Chats list nor a tab on the strip`);
}

/**
 * **Ends chat `session` in `plane`** (the project in front where none is named), as a person
 * does: brings it forward, presses the `×` of the tab in front (its own, since its pane is in
 * front), answers the question's Close, and waits until the core no longer runs it. A chat
 * already gone is left alone.
 *
 * A split tab's `×` ends the tab's own chat, and the chat split beside it goes on in a tab of
 * its own: so where the chat is still running after one press, it is brought forward and its
 * tab's `×` pressed again, three presses at most.
 */
export async function endChatById(session: number, plane?: string): Promise<void> {
  const where = plane ?? (await projectInFront());
  const most = 3;
  for (let pressed = 1; pressed <= most; pressed++) {
    if (!(await runningIn(where)).includes(session)) return;
    const asked = await pressItsTabsEnd(session);
    const gone = await browser
      .waitUntil(async () => !(await runningIn(where)).includes(session), {
        timeout: pressed < most ? 10_000 : 20_000,
        interval: 200,
      })
      .then(() => true)
      .catch(() => false);
    if (gone) return;
    if (pressed === most)
      throw new Error(
        `chat ${session} was still running after ${asked} was answered Close ${most} times`,
      );
  }
}

/**
 * Brings chat `session` forward, presses the `×` of the tab in front, answers the question's
 * Close and waits until the question has gone; answers the `×`'s name.
 */
async function pressItsTabsEnd(session: number): Promise<string> {
  await bringChatForward(session);
  const asked = await browser.execute((selected: string) => {
    const closer = document
      .querySelector(selected)
      ?.closest("span.tab")
      ?.querySelector<HTMLElement>("button.closer:not([data-minimise])");
    closer?.click();
    return closer?.getAttribute("aria-label") ?? null;
  }, SELECTED_TAB);
  if (asked === null) throw new Error(`the tab in front for chat ${session} has no × that ends it`);
  await browser.waitUntil(
    async () =>
      browser.execute(
        (about: string) => {
          // Only the question this × asked, about its chat by name: a stray dialog never ends
          // another chat.
          const asking = document.querySelector('[role="alertdialog"]');
          if (!asking?.textContent?.includes(about)) return false;
          const close = [...asking.querySelectorAll<HTMLButtonElement>("button")].find(
            (button) => button.textContent?.trim() === "Close",
          );
          close?.click();
          return close !== undefined;
        },
        asked.replace(/^End chat /, ""),
      ),
    { timeout: 20_000, interval: 100, timeoutMsg: `${asked} never asked whether to end it` },
  );
  await browser.waitUntil(
    async () => browser.execute(() => document.querySelector('[role="alertdialog"]') === null),
    { timeout: 20_000, interval: 100, timeoutMsg: `the question ${asked} asked never went` },
  );
  return asked;
}

/**
 * Ends each chat a spec started, in order, each one already gone left alone. Every chat is
 * tried even when one before it fails, and the first failure is thrown after, so an `after`
 * leaves as little behind as it can.
 */
export async function endChatsStartedHere(
  chats: readonly (Started | number | undefined)[],
): Promise<void> {
  let first: unknown;
  for (const chat of chats) {
    if (chat === undefined) continue;
    try {
      if (typeof chat === "number") await endChatById(chat);
      else await endChatById(chat.session, chat.plane);
    } catch (trouble) {
      first ??= trouble;
    }
  }
  if (first !== undefined) throw first;
}
