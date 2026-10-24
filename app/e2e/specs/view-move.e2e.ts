// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { closeMenus } from "../acting.js";
import { pressMenuPath, ROW } from "../menus.js";
import { theWindowHoldsTheRail, WINDOW_1280, windowAt } from "../rail.js";

/**
 * **A view moved to its half of the Panel from the keyboard** (#1752, #1740, P-3), in the built
 * app: the Search view's icon gets the keyboard, its menu is opened with Shift+F10 (a scenario
 * cannot right-click: `docs/ui-primitives.md`), _Move to panel_ moves it into the Panel's left
 * half, and the real sizes are read there: the half's line and its Lower icons fit, and nothing
 * overflows the window.
 *
 * **The key a person presses, on the icon that has the keyboard, and the menu's own state
 * waited on** (#1973): the keydown Shift+F10 makes, dispatched from the page to the icon while
 * it holds the keyboard, until the icon says its menu is open ({@link openMenuOf}). The
 * driver's own Shift+F10 arrives without Shift held and opens nothing. Every focus is given from
 * the page. **It leaves the window as it found it**: Search is moved back to the sidebar by its
 * menu there, and the left side shows Chats again, picked as a person picks it and waited for.
 */

const NAVIGATION = '[role="tablist"][aria-label="Navigation"]';
const LOWER_LEFT = '[role="tablist"][aria-label="Panel, left half"]';
const HALF = '[data-panel][id="region-panel"] .panel-half[data-side="left"]';
const MENU = '[role="menu"]';

/**
 * **Opens the menu of the view icon `name` in the bar `bar` with the keydown a person's
 * Shift+F10 makes** (#1973), and waits on the icon's own menu state: the menu family marks the
 * element a context menu is open on `data-open` while it is up.
 *
 * **The key is dispatched from the page, to the icon, in the same step that gives the icon the
 * keyboard.** The menu opens only for a Shift+F10 whose target is the icon itself
 * (`ui/menu/ContextMenu.tsx`). The driver's Shift+F10 cannot be that key: measured on
 * both engines' scenario runs, the WebDriver plugin's F10 reaches the icon with `shiftKey`
 * false, so it opens nothing. And a key sent to whatever has the keyboard a moment later misses
 * whenever the window has moved the keyboard in between — a moved view takes it in a frame of
 * its own (`PlaneView`'s `moveSideView`) — which is how this spec failed on main. Focus, the
 * check that the icon holds it, and the key are one synchronous script, so nothing runs between
 * them. A try that opens nothing is tried again; an icon whose menu is open is left alone, since
 * focusing it again would close the menu.
 */
async function openMenuOf(bar: string, name: string): Promise<void> {
  const icon = `${bar} [role="tab"][aria-label="${name}"]`;
  await browser.waitUntil(
    async () =>
      browser.execute((css: string) => {
        const tab = document.querySelector<HTMLElement>(css);
        if (tab === null) return false;
        if (tab.hasAttribute("data-open")) return true;
        tab.focus();
        if (document.activeElement !== tab) return false;
        tab.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "F10",
            shiftKey: true,
            bubbles: true,
            cancelable: true,
          }),
        );
        return false;
      }, icon),
    {
      timeout: 20_000,
      interval: 500,
      timeoutMsg: `Shift+F10 on the ${name} icon in ${bar} never opened its menu`,
    },
  );
}

/**
 * Presses the open menu's row `name` **by its name** (`ROW.toPanel`, `e2e/menus.ts`), never by
 * words written down here (#2387), and waits for the menu to close.
 */
async function pressRow(name: string): Promise<void> {
  await pressMenuPath(name);
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
  // At 1280×800, where this was held to fit before the default window grew (`rail.ts`,
  // #3077); the project rail hides itself there beside both sidebars.
  before(() => windowAt(WINDOW_1280));
  after(() => theWindowHoldsTheRail());

  this.timeout(180_000);

  before(async () => {
    await $(NAVIGATION).waitForExist({ timeout: 30_000 });
    await $(LOWER_LEFT).waitForExist({ timeout: 30_000 });
  });

  after(async () => {
    // A menu a failure left open is over everything the hook reaches for.
    await closeMenus();
    // Search back in the left sidebar, however far this spec got, and the side on Chats.
    if ((await iconsOf(LOWER_LEFT)).includes("Search")) {
      await openMenuOf(LOWER_LEFT, "Search");
      await pressRow(ROW.toSidebar);
    }
    await showChatsAgain();
  });

  it("moves Search into the Panel's left half, open there", async () => {
    expect(await iconsOf(NAVIGATION)).toContain("Search");

    await openMenuOf(NAVIGATION, "Search");
    await pressRow(ROW.toPanel);

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
        // The left bar is a column beside the project rail (#3144).
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
    await openMenuOf(LOWER_LEFT, "Search");
    await pressRow(ROW.toSidebar);

    await browser.waitUntil(async () => (await iconsOf(NAVIGATION)).includes("Search"), {
      timeout: 20_000,
      timeoutMsg: "Search never came back to the left sidebar",
    });
    expect(await iconsOf(LOWER_LEFT)).not.toContain("Search");
  });
});
