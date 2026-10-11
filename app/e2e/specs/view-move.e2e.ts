// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";

/**
 * **A view moved to its half of the Panel from the keyboard** (#1752, #1740, P-3), in the built
 * app: the Search view's icon gets the keyboard, its menu is opened with Shift+F10 (a scenario
 * cannot right-click: `docs/ui-primitives.md`), _Move to panel_ moves it into the Panel's left
 * half, and the real sizes are read there: the half's line and its Lower icons fit, and nothing
 * overflows the window.
 *
 * **Written defensively, CI first**: the WebDriver plugin's keys are untrusted synthetic events,
 * so where the driver's Shift+F10 opens no menu the same key is dispatched from the page, where
 * the menu's own key handler hears it as it hears a person's. Every focus is given from the
 * page. **It leaves the window as it found it**: Search is moved back to the sidebar by its menu
 * there, and the left side shows Chats again, picked as a person picks it and waited for.
 */

const NAVIGATION = '[role="tablist"][aria-label="Navigation"]';
const LOWER_LEFT = '[role="tablist"][aria-label="Panel, left half"]';
const HALF = '[data-panel][id="region-panel"] .panel-half[data-side="left"]';
const MENU = '[role="menu"]';

/** Gives the keyboard to the view icon `name` in the bar `bar`, from the page. */
async function focusIcon(bar: string, name: string): Promise<void> {
  const found = await browser.execute(
    (selector: string, called: string) => {
      const tab = document.querySelector<HTMLElement>(
        `${selector} [role="tab"][aria-label="${called}"]`,
      );
      tab?.focus();
      return tab !== null && document.activeElement === tab;
    },
    bar,
    name,
  );
  if (!found) throw new Error(`no ${name} icon in ${bar} took the keyboard`);
}

/** Opens the focused icon's menu from the keyboard: the driver's Shift+F10, else the page's. */
async function openMenuFromTheKeyboard(): Promise<void> {
  await browser.keys(["Shift", "F10"]);
  const opened = await $(MENU)
    .waitForDisplayed({ timeout: 3_000 })
    .then(() => true)
    .catch(() => false);
  if (opened) return;
  await browser.execute(() => {
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "F10", shiftKey: true, bubbles: true, cancelable: true }),
    );
  });
  await $(MENU).waitForDisplayed({ timeout: 10_000 });
}

/** Presses the open menu's row `title`. */
async function pressRow(title: string): Promise<void> {
  const row = await $(
    `//*[@role="menu"]//*[@role="menuitem"][contains(normalize-space(.), "${title}")]`,
  );
  await row.waitForClickable({ timeout: 10_000 });
  await row.click();
  await expect($(MENU)).not.toBeDisplayed();
}

/** The icons a bar shows, by name. */
const iconsOf = (bar: string) =>
  browser.execute(
    (selector: string) =>
      [...document.querySelectorAll(`${selector} [role="tab"]`)].map(
        (tab) => tab.getAttribute("aria-label") ?? "",
      ),
    bar,
  );

/** Closes a menu left open, from the keyboard: the driver's Escape, else the page's. */
async function closeAnOpenMenu(): Promise<void> {
  if (
    !(await $(MENU)
      .isDisplayed()
      .catch(() => false))
  )
    return;
  await browser.keys(["Escape"]);
  const closed = await $(MENU)
    .waitForDisplayed({ timeout: 3_000, reverse: true })
    .then(() => true)
    .catch(() => false);
  if (closed) return;
  await browser.execute(() => {
    (document.activeElement ?? document.body).dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    );
  });
  await $(MENU).waitForDisplayed({ timeout: 10_000, reverse: true });
}

/**
 * **The left side back on Chats, as a person picks it** (#1759): a pointer press on the Chats
 * icon (WebKitGTK sends only mousedown and click, and the icon is picked on its click), and the
 * pick waited for, so the next spec never starts on Search. Where the driver's press does not
 * land, the icon's own click from the page. An icon already picked is left alone: pressed
 * again, it would put the side away.
 */
async function showChatsAgain(): Promise<void> {
  const chats = await $(`${NAVIGATION} [role="tab"][aria-label="Chats"]`);
  if (!(await chats.isExisting())) return;
  const picked = async () => (await chats.getAttribute("aria-selected")) === "true";
  if (await picked()) return;
  await chats.click().catch(() => undefined);
  const took = await browser
    .waitUntil(picked, { timeout: 5_000 })
    .then(() => true)
    .catch(() => false);
  if (took) return;
  if (!(await picked()))
    await browser.execute((tab: HTMLElement) => tab.click(), chats as unknown as HTMLElement);
  await browser.waitUntil(picked, {
    timeout: 10_000,
    timeoutMsg: "the left side never came back to Chats",
  });
}

describe("moving a view from its icon's menu, by keyboard", function () {
  this.timeout(180_000);

  before(async () => {
    await $(NAVIGATION).waitForExist({ timeout: 30_000 });
    await $(LOWER_LEFT).waitForExist({ timeout: 30_000 });
  });

  after(async () => {
    // A menu a failure left open is over everything the hook reaches for.
    await closeAnOpenMenu();
    // Search back in the left sidebar, however far this spec got, and the side on Chats.
    if ((await iconsOf(LOWER_LEFT)).includes("Search")) {
      await focusIcon(LOWER_LEFT, "Search");
      await openMenuFromTheKeyboard();
      await pressRow("Move to sidebar");
    }
    await showChatsAgain();
  });

  it("moves Search into the Panel's left half, open there", async () => {
    expect(await iconsOf(NAVIGATION)).toContain("Search");

    await focusIcon(NAVIGATION, "Search");
    await openMenuFromTheKeyboard();
    await pressRow("Move to panel");

    await browser.waitUntil(async () => (await iconsOf(LOWER_LEFT)).includes("Search"), {
      timeout: 20_000,
      timeoutMsg: "Search never reached the Panel's left half",
    });
    expect(await iconsOf(NAVIGATION)).not.toContain("Search");
    await expect($(`${HALF} [role="tabpanel"][data-view="search"]`)).toBeDisplayed();
  });

  it("fits the half's line, its Lower icons and the window, with nothing overflowing", async () => {
    const sizes = await browser.execute(
      (half: string, lower: string) => {
        const over = (el: Element | null) =>
          el === null
            ? null
            : { wide: el.scrollWidth - el.clientWidth, tall: el.scrollHeight - el.clientHeight };
        const head = document.querySelector(`${half} .half-head`);
        const column = document.querySelector(".activity-column[data-side='left']");
        const icons = document.querySelector(lower)?.getBoundingClientRect();
        const bar = column?.getBoundingClientRect();
        return {
          head: over(head),
          headHeight: head?.getBoundingClientRect().height ?? 0,
          title: head?.querySelector("h2")?.textContent ?? "",
          column: over(column),
          iconsInside: icons !== undefined && bar !== undefined && icons.bottom <= bar.bottom + 1,
          page: {
            wide: document.documentElement.scrollWidth - window.innerWidth,
            tall: document.documentElement.scrollHeight - window.innerHeight,
          },
        };
      },
      HALF,
      LOWER_LEFT,
    );

    expect(sizes.title).toBe("Search");
    expect(sizes.head).not.toBeNull();
    expect(sizes.head?.wide).toBeLessThanOrEqual(1);
    expect(sizes.headHeight).toBeGreaterThan(0);
    expect(sizes.column?.tall).toBeLessThanOrEqual(1);
    expect(sizes.iconsInside).toBe(true);
    expect(sizes.page.wide).toBeLessThanOrEqual(0);
    expect(sizes.page.tall).toBeLessThanOrEqual(0);
  });

  it("moves it back to the sidebar from its Lower icon's menu", async () => {
    await focusIcon(LOWER_LEFT, "Search");
    await openMenuFromTheKeyboard();
    await pressRow("Move to sidebar");

    await browser.waitUntil(async () => (await iconsOf(NAVIGATION)).includes("Search"), {
      timeout: 20_000,
      timeoutMsg: "Search never came back to the left sidebar",
    });
    expect(await iconsOf(LOWER_LEFT)).not.toContain("Search");
  });
});
