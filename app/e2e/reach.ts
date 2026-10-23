// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser } from "@wdio/globals";
import { projectInFront, runningIn } from "./running.js";
import { endChatsStartedHere } from "./started.js";

/**
 * **Getting somewhere in the window without trusting the driver's keys or its pointer**, for
 * the specs written CI first (lanes of 2026-10-12).
 *
 * The WebDriver plugin's keys are untrusted synthetic events, so a key that does something only
 * through the engine's own default action does nothing here; WebKitGTK sends a press as
 * mousedown and click alone; and an element with no width cannot be pressed at all. So every
 * press below is the element's own `click()` from the page, every focus is given from the page,
 * and the one key used, the palette's F2, falls back to the same key dispatched on the window,
 * where the palette's own handler hears it as it hears a person's.
 */

// Answered from the leaf it lives in, which reaches no chat: kept here for the specs that
// import it from this file.
export { projectInFront };

const TABS = '[data-strip="Tabs"]';
const WORKSPACES = '[data-strip="Workspaces"]';
const PALETTE = '[role="dialog"][aria-label="Command palette"]';

/** The names on the tab strip, left to right. */
export async function tabNames(): Promise<string[]> {
  return browser.execute(
    (strip: string) =>
      [...(document.querySelector(strip)?.querySelectorAll('[role="tab"]') ?? [])].map(
        (tab) => tab.querySelector(".tab-name")?.textContent ?? "",
      ),
    TABS,
  );
}

/** Brings the tab called `name` forward, by its own click from the page. */
export async function bringForward(name: string): Promise<void> {
  const found = await browser.execute(
    (strip: string, called: string) => {
      const tab = [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)].find(
        (one) => one.querySelector(".tab-name")?.textContent === called,
      );
      tab?.click();
      return tab !== undefined;
    },
    TABS,
    name,
  );
  if (!found) throw new Error(`no tab called ${name} on the strip`);
  await browser.waitUntil(
    async () =>
      browser.execute(
        (strip: string, called: string) =>
          [...document.querySelectorAll(`${strip} [role="tab"]`)].some(
            (one) =>
              one.querySelector(".tab-name")?.textContent === called &&
              one.getAttribute("aria-selected") === "true",
          ),
        TABS,
        name,
      ),
    { timeout: 10_000, interval: 100, timeoutMsg: `the ${name} tab never came forward` },
  );
}

/** The chat the pane in front shows, by its number, once there is one. */
export async function sessionInFront(): Promise<number> {
  let session = -1;
  await browser.waitUntil(
    async () => {
      session = await browser.execute(() => {
        const pane = [...document.querySelectorAll<HTMLElement>('[data-testid="pane"]')].find(
          (one) => one.getBoundingClientRect().width > 0,
        );
        return Number(pane?.getAttribute("data-session") ?? -1);
      });
      return session >= 0;
    },
    { timeout: 20_000, interval: 100, timeoutMsg: "no chat's pane was drawn in front" },
  );
  return session;
}

/**
 * **Ends every chat the project in front runs that `before` did not list** (#1796): `before` is
 * its `running_sessions`, read before the spec started anything. Each is ended by its number
 * through the window (`started.ts`), never by its tab's name, so a chat of the same name that
 * was there first is never the one pressed.
 */
export async function endTheSessionsStartedSince(before: readonly number[]): Promise<void> {
  const plane = await projectInFront();
  const started = (await runningIn(plane)).filter((one) => !before.includes(one));
  await endChatsStartedHere(started.map((session) => ({ plane, session, name: "" })));
}

/**
 * Closes every view tab that was not on the strip in `before`, by its own `Close <name>`, and
 * waits until the strip is as it was: one app process serves the whole run. A view tab ends
 * nothing and closes without a question. **It ends no chat**: a chat is ended by its number
 * ({@link endTheSessionsStartedSince}) first, and a chat's tab still on the strip fails here.
 */
export async function closeTheViewTabsOpenedSince(before: readonly string[]): Promise<void> {
  for (let round = 0; round < 5; round++) {
    const left = (await tabNames()).filter((tab) => !before.includes(tab));
    if (left.length === 0) break;
    for (const name of left)
      await browser.execute(
        (strip: string, called: string) => {
          document
            .querySelector<HTMLElement>(`${strip} button[aria-label="Close ${CSS.escape(called)}"]`)
            ?.click();
        },
        TABS,
        name,
      );
    await browser.pause(500);
  }
  let left: string[] = [];
  await browser
    .waitUntil(
      async () => {
        left = (await tabNames()).filter((tab) => !before.includes(tab));
        return left.length === 0;
      },
      { timeout: 20_000, interval: 250 },
    )
    .catch(() => {
      throw new Error(`tabs this spec opened were left on the strip: ${left.join(", ")}`);
    });
}

/** The workspace strip's tabs, by what each says: the plane root's icon tab by its name. */
export async function workspaceTabs(): Promise<{ name: string; selected: boolean }[]> {
  return browser.execute(
    (strip: string) =>
      [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)].map((tab) => ({
        name:
          tab.querySelector(".workspace-name")?.textContent ?? tab.getAttribute("aria-label") ?? "",
        selected: tab.getAttribute("aria-selected") === "true",
      })),
    WORKSPACES,
  );
}

/** Focuses the workspace tab at `at` on the strip (the order {@link workspaceTabs} reads). */
export async function focusWorkspaceAt(at: number): Promise<void> {
  await browser.execute(
    (strip: string, index: number) => {
      document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)[index]?.click();
    },
    WORKSPACES,
    at,
  );
  await browser.waitUntil(async () => (await workspaceTabs())[at]?.selected === true, {
    timeout: 10_000,
    interval: 100,
    timeoutMsg: `the workspace tab at ${at} never took the focus`,
  });
}

/** Focuses the workspace called `name`, once the strip lists it. */
export async function focusWorkspace(name: string): Promise<void> {
  let at = -1;
  await browser.waitUntil(
    async () => {
      at = (await workspaceTabs()).findIndex((tab) => tab.name === name);
      return at >= 0;
    },
    { timeout: 30_000, interval: 250, timeoutMsg: `the strip never listed ${name}` },
  );
  await focusWorkspaceAt(at);
}

/**
 * **Runs the palette's row `id`**, found by typing `typed`: F2 opens the palette, and where the
 * driver's F2 opens nothing the same key is dispatched on the window, where the palette listens.
 * The row is pressed by its own click.
 */
export async function runFromThePalette(typed: string, id: string): Promise<void> {
  const palette = await $(PALETTE);
  if (!(await palette.isDisplayed().catch(() => false))) {
    await browser.keys(["F2"]);
    const opened = await palette
      .waitForDisplayed({ timeout: 3_000 })
      .then(() => true)
      .catch(() => false);
    if (!opened) {
      await browser.execute(() => {
        window.dispatchEvent(
          new KeyboardEvent("keydown", { key: "F2", bubbles: true, cancelable: true }),
        );
      });
      await palette.waitForDisplayed({ timeout: 10_000 });
    }
  }
  const box = await $("#palette-query");
  await box.waitForDisplayed({ timeout: 10_000 });
  await box.addValue(typed);
  await browser.waitUntil(
    async () =>
      browser.execute((row: string) => {
        const found = document.getElementById(`palette-row-${row}`);
        found?.click();
        return found !== null;
      }, id),
    { timeout: 10_000, interval: 100, timeoutMsg: `the palette never offered ${id}` },
  );
  const shut = await palette
    .waitForDisplayed({ reverse: true, timeout: 5_000 })
    .then(() => true)
    .catch(() => false);
  if (!shut)
    await browser.execute(() => {
      document.activeElement?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
      );
    });
}
