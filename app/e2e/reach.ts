// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, $$, browser } from "@wdio/globals";
import { projectInFront, runningIn } from "./running.js";
import { endChatsStartedHere } from "./started.js";
import { goToWorkspaceAt, workspacesListed } from "./titleBarWorkspaces.js";

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

export const TABS = '[data-strip="Tabs"]';
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

/**
 * **Every tab on the strip in front, by name**: the ones it draws, left to right, then the ones
 * its "Show N tabs" hides (ADR 0039), read from that menu as a person reads them, and the menu
 * closed again. A strip with more tabs than its room draws the selected one and hides others, so
 * the drawn ones are not all there are: on the macOS runner, where a tab's words take more room,
 * a strip whose earlier files left tabs on it hid a chat's tab (#4758's merge-queue run).
 */
export async function everyTabName(): Promise<string[]> {
  const drawn = await tabNames();
  const opened = await browser.execute(() => {
    const more = [...document.querySelectorAll<HTMLElement>("button[aria-haspopup='menu']")].find(
      (one) =>
        /^Show \d+ tabs? the strip is not showing/.test(one.getAttribute("aria-label") ?? ""),
    );
    more?.click();
    return more !== undefined;
  });
  if (!opened) return drawn;
  const menu = '[role="menu"][aria-label$="the strip is not showing"]';
  await $(menu).waitForExist({ timeout: 10_000 });
  const hidden = await browser.execute(
    (css: string) =>
      [...(document.querySelector(css)?.querySelectorAll('[role="menuitem"]') ?? [])].map(
        (row) => row.querySelector(".tab-name")?.textContent ?? "",
      ),
    menu,
  );
  await browser.keys(["Escape"]);
  // Escape reaches the menu only where the keyboard went into it. Opened by a click from the
  // page, it may not have: a second press of its own button closes it then, as a person's does.
  // Since the 160px floor (#5341) a strip hides tabs it used to draw, so this menu is opened here
  // more often than it was.
  const closed = await $(menu)
    .waitForExist({ timeout: 2_000, reverse: true })
    .catch(() => false);
  if (!closed)
    await browser.execute(() => {
      [...document.querySelectorAll<HTMLElement>("button[aria-haspopup='menu']")]
        .find((one) =>
          /^Show \d+ tabs? the strip is not showing/.test(one.getAttribute("aria-label") ?? ""),
        )
        ?.click();
    });
  await $(menu).waitForExist({ timeout: 10_000, reverse: true });
  return [...drawn, ...hidden];
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

/** What the strip's kept empty tab is called (`tabs.EMPTY_TAB`, #4755). */
export const KEPT_EMPTY = "New chat";

/**
 * **The tabs on `now` that were not on `before`**: by name, and as many times as each was added,
 * so a second tab of a name the strip already had is one. **One kept empty tab is never one**
 * (#4756): closing a strip's last tab keeps one empty pane, named "New chat", and a fresh strip
 * starts with one, so one "New chat" is always the strip's own. **Past that one, or past as many
 * as the strip began with, a "New chat" is a tab left open** (#5586): the strip's `+`, or a chat's pane
 * emptied, leaves an unnamed tab, and a file that leaves them fills the strip of every file
 * after it. The one rule both a spec's own `after` and `tabsLeft.ts`'s guard use.
 */
export function openedSince(before: readonly string[], now: readonly string[]): string[] {
  const had = new Map<string, number>();
  for (const name of before) had.set(name, (had.get(name) ?? 0) + 1);
  had.set(KEPT_EMPTY, Math.max(had.get(KEPT_EMPTY) ?? 0, 1));
  const opened: string[] = [];
  for (const name of now) {
    const kept = had.get(name) ?? 0;
    if (kept > 0) had.set(name, kept - 1);
    else opened.push(name);
  }
  return opened;
}

/**
 * Presses each named tab's own `Close <name>` on the strip in front, from the page: a name
 * called twice presses two tabs of that name, the strip's last ones, since unnamed "New chat"
 * tabs share one name.
 */
export async function pressClose(names: readonly string[]): Promise<void> {
  await browser.execute(
    (strip: string, called: string[]) => {
      const times = new Map<string, number>();
      for (const name of called) times.set(name, (times.get(name) ?? 0) + 1);
      for (const [name, count] of times) {
        const buttons = [
          ...document.querySelectorAll<HTMLElement>(
            `${strip} button[aria-label="Close ${CSS.escape(name)}"]`,
          ),
        ];
        for (const button of buttons.slice(-count)) button.click();
      }
    },
    TABS,
    [...names],
  );
}

/**
 * Closes every view tab that was not on the strip in `before` ({@link openedSince}), by its own
 * `Close <name>`, and waits until the strip is as it was: one app process serves the whole run.
 * A view tab ends nothing and closes without a question. **It ends no chat**: a chat is ended by
 * its number ({@link endTheSessionsStartedSince}) first, and a chat's tab still on the strip
 * fails here.
 */
export async function closeTheViewTabsOpenedSince(before: readonly string[]): Promise<void> {
  for (let round = 0; round < 5; round++) {
    const left = openedSince(before, await tabNames());
    if (left.length === 0) break;
    await pressClose(left);
    await browser.pause(500);
  }
  let left: string[] = [];
  await browser
    .waitUntil(
      async () => {
        left = openedSince(before, await tabNames());
        return left.length === 0;
      },
      { timeout: 20_000, interval: 250 },
    )
    .catch(() => {
      throw new Error(`tabs this spec opened were left on the strip: ${left.join(", ")}`);
    });
}

/**
 * **The strip in front holds nothing but its kept empty tab, if that** (#5131): what a spec file
 * that counts on an exact strip waits for as it begins and as it ends. Every file closes what it
 * opens, and `tabsLeft.ts` fails one that does not, so there is nothing to tolerate. Tabs a file
 * before this one is still closing get the moment they take.
 */
export async function untilTheStripIsBare(spec: string): Promise<void> {
  let left: string[] = [];
  await browser
    .waitUntil(
      async () => {
        left = openedSince([], await tabNames());
        return left.length === 0;
      },
      { timeout: 20_000, interval: 250 },
    )
    .catch(() => {
      throw new Error(`${spec}: the strip should be bare, and holds ${left.join(", ")}`);
    });
}

/**
 * **This `describe` closes the view tabs it opens** (#5111, #5131): writes down the strip before
 * its specs and closes what is new after them, by {@link closeTheViewTabsOpenedSince}. A `before`
 * and an `after` registered where it is called, and hooks run in the order they are registered,
 * so call it below any `after` that has to come first, such as one ending the file's chats. A
 * strip never read, because a `before` above failed, is not closed.
 */
export function closesTheTabsItOpens(): void {
  // `null` until the strip is read: a `before` above that failed skips this one, and its `after`
  // then closes nothing rather than every view tab on the strip.
  let wereOpen: string[] | null = null;
  before(async () => {
    wereOpen = await tabNames();
  });
  after(async () => {
    if (wereOpen !== null) await closeTheViewTabsOpenedSince(wereOpen);
  });
}

/**
 * **Pane notices minimised again, where this file expanded them** (#5586). Minimised or expanded
 * is the window's one choice, kept (`paneNoticesPref.ts`), so a file that presses a pane's bell
 * to show what it raised leaves every later file's notices expanded: `skill-compat.e2e.ts` then
 * found its bell open in a single-process run. A file that presses it sets
 * `window.__e2eNoticesExpanded`, and calls this in its `after` while a pane is still open, since
 * only a pane's bell sets the choice.
 */
export async function minimiseNoticesExpandedHere(): Promise<void> {
  const pressed = await browser.execute(() => {
    const page = window as unknown as Record<string, boolean | undefined>;
    if (page.__e2eNoticesExpanded !== true) return true;
    page.__e2eNoticesExpanded = undefined;
    const bell = document.querySelector<HTMLElement>('[data-way="bell"][aria-expanded="true"]');
    bell?.click();
    return bell !== null || document.querySelector('[data-way="bell"]') !== null;
  });
  // Said, never thrown: it runs first in an `after` that still has chats to end.
  if (!pressed) {
    console.warn("no pane's bell was left to minimise the notices this file expanded");
    return;
  }
  await browser
    .waitUntil(
      () =>
        browser.execute(() => !document.querySelector('[data-way="bell"][aria-expanded="true"]')),
      { timeout: 5_000, interval: 100, timeoutMsg: "the pane notices stayed expanded" },
    )
    .catch((trouble: unknown) => console.warn(trouble));
}

/** Every workspace of the project in front, as the title bar's ⌄ lists them (#2916): the plane
 *  root first (`Plane root`), the one in front `selected`. */
export async function workspaceTabs(): Promise<{ name: string; selected: boolean }[]> {
  return workspacesListed();
}

/** Focuses the workspace at `at` (the order {@link workspaceTabs} reads), by its row in the ⌄. */
export async function focusWorkspaceAt(at: number): Promise<void> {
  await goToWorkspaceAt(at);
  await browser.waitUntil(async () => (await workspaceTabs())[at]?.selected === true, {
    timeout: 10_000,
    interval: 100,
    timeoutMsg: `the workspace at ${at} never came to the front`,
  });
}

/** Focuses the workspace called `name`, once the project lists it. */
export async function focusWorkspace(name: string): Promise<void> {
  let at = -1;
  await browser.waitUntil(
    async () => {
      at = (await workspaceTabs()).findIndex((tab) => tab.name === name);
      return at >= 0;
    },
    { timeout: 30_000, interval: 250, timeoutMsg: `the title bar never listed ${name}` },
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

/**
 * **The button called `text` that is on screen** (#3694): a view the sidebar has shown once is
 * kept drawn while another is shown, so a label two views share (the Skills view's Open Skill
 * studio and New skill…, beside Settings' and the studio's own) may be held by one put away. The
 * first one displayed, waited for until `timeout`.
 */
export async function shownButton(text: string, timeout = 20_000): Promise<WebdriverIO.Element> {
  let found: WebdriverIO.Element | undefined;
  await browser.waitUntil(
    async () => {
      for (const one of await $$(`button=${text}`)) {
        if (await one.isDisplayed()) {
          found = one;
          return true;
        }
      }
      return false;
    },
    { timeout, timeoutMsg: `no ${text} button on screen` },
  );
  if (found === undefined) throw new Error(`no ${text} button on screen`);
  return found;
}
