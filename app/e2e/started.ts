// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { pressAndStart, showView } from "./opening.js";
import { projectInFront } from "./reach.js";
import { ask } from "./switching.js";

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

/** The chats `plane` is running, by number. */
export async function runningIn(plane: string): Promise<number[]> {
  return ask<number[]>("running_sessions", { plane });
}

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

/** The chat whose pane is drawn in front, by its number, or -1 where none is: read at once. */
export async function paneInFront(): Promise<number> {
  return browser.execute(() => {
    const pane = [...document.querySelectorAll<HTMLElement>('[data-testid="pane"]')].find(
      (one) => one.getBoundingClientRect().width > 0,
    );
    return Number(pane?.getAttribute("data-session") ?? -1);
  });
}

/** Waits until the pane in front is chat `session`'s. */
async function untilInFront(session: number, timeout = 20_000): Promise<void> {
  let seen = -1;
  await browser.waitUntil(
    async () => {
      seen = await paneInFront();
      return seen === session;
    },
    {
      timeout,
      interval: 100,
      timeoutMsg: `chat ${session}'s pane never came in front (chat ${seen}'s was)`,
    },
  );
}

/**
 * **Brings chat `session` forward**, by its row in the Chats list: the row carries the number,
 * and a press on it shows its chat. Where the list does not draw the row (a scope or a filter a
 * spec left picked), each chat tab on the strip is brought forward in turn until the pane in
 * front is that chat's: never a tab found by its name.
 */
export async function bringChatForward(session: number): Promise<void> {
  if ((await paneInFront()) === session) return;
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
      .waitUntil(async () => (await paneInFront()) === session, {
        timeout: 2_000,
        interval: 100,
      })
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
 */
export async function endChatById(session: number, plane?: string): Promise<void> {
  const where = plane ?? (await projectInFront());
  if (!(await runningIn(where)).includes(session)) return;
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
  await browser.waitUntil(async () => !(await runningIn(where)).includes(session), {
    timeout: 20_000,
    interval: 200,
    timeoutMsg: `chat ${session} was still running after ${asked} was answered Close`,
  });
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
