// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";

/**
 * **The four corners, in the real window** (#2917, spec #2897 R116–R128, N36): the left bar is
 * tabs across the top of the side panel, the right bar a 44px column; a top icon switches its
 * side's panel and a bottom one opens its half of the bottom panel; only the part that opened
 * moves, and closing has no motion. Measured where jsdom lays nothing out: the boxes, the
 * animations the page is running, and that nothing beside a chosen icon moves.
 */

const NAVIGATION = '[role="tablist"][aria-label="Navigation"]';
const ATTENTION = '[role="tablist"][aria-label="Attention"]';
const LOWER_RIGHT = '[role="tablist"][aria-label="Panel, right half"]';
const tab = (bar: string, name: string) => `${bar} [role="tab"][aria-label="${name}"]`;

type Box = { left: number; top: number; width: number; height: number };

async function boxOf(css: string): Promise<Box | null> {
  return browser.execute((selector: string) => {
    const it = document.querySelector(selector);
    if (it === null) return null;
    const { left, top, width, height } = it.getBoundingClientRect();
    return { left, top, width, height };
  }, css);
}

/** The animations running on what `css` names, by their keyframes' or effect's name. */
async function moving(css: string): Promise<string[]> {
  return browser.execute((selector: string) => {
    const it = document.querySelector(selector);
    if (it === null) return [];
    return it
      .getAnimations()
      .map((one) => (one as CSSAnimation).animationName ?? one.constructor.name);
  }, css);
}

async function press(css: string): Promise<void> {
  const it = await $(css);
  await it.waitForExist({ timeout: 30_000 });
  await it.click();
}

describe("the four corners", function () {
  this.timeout(180_000);

  before(async () => {
    await $(NAVIGATION).waitForExist({ timeout: 30_000 });
    await $(ATTENTION).waitForExist({ timeout: 30_000 });
  });

  it("draws the left bar as 32×30 tabs across the side panel's top, its bottom group at the end", async () => {
    const chats = await boxOf(tab(NAVIGATION, "Chats"));
    const changes = await boxOf('.slot-left [role="tab"][aria-label="Changes"]');
    const side = await boxOf(".slot-left");
    expect(chats).not.toBeNull();
    expect(chats?.width).toBe(32);
    expect(chats?.height).toBe(30);
    // One row, inside the side panel, Changes at its end.
    expect(changes?.top).toBe(chats?.top);
    expect((changes?.left ?? 0) + (changes?.width ?? 0)).toBeGreaterThan(
      (side?.left ?? 0) + (side?.width ?? 0) - 24,
    );
  });

  it("draws the right bar as a 44px column of 36px icons", async () => {
    const column = await boxOf(".activity-column[data-side='right']");
    const inbox = await boxOf(tab(ATTENTION, "Inbox"));
    expect(column?.width).toBe(44);
    expect(inbox?.width).toBe(36);
    expect(inbox?.height).toBe(36);
  });

  it("switches the side view, and a press of the open one leaves it out", async () => {
    await press(tab(NAVIGATION, "Explorer"));
    await browser.waitUntil(
      async () => (await $(tab(NAVIGATION, "Explorer")).getAttribute("aria-selected")) === "true",
      { timeout: 10_000, timeoutMsg: "Explorer never opened" },
    );
    await press(tab(NAVIGATION, "Explorer"));
    expect(await $(tab(NAVIGATION, "Explorer")).getAttribute("aria-selected")).toBe("true");
    await press(tab(NAVIGATION, "Chats"));
  });

  it("brings the right panel in only when it opens, and takes it away with no motion", async () => {
    // Whichever right view a spec before this one left open, closed by the panel's ×.
    if (await $(`${ATTENTION} [role="tab"][aria-selected="true"]`).isExisting())
      await press(".slot-right .side-close");
    await browser.waitUntil(async () => ((await boxOf(".slot-right"))?.width ?? 0) <= 1, {
      timeout: 10_000,
      timeoutMsg: "the right panel never closed",
    });
    expect(await moving(".slot-right")).toEqual([]);

    await press(tab(ATTENTION, "Inbox"));
    // 300px where the window has room, else its 20% (R124).
    const wide = await browser.execute(() => document.documentElement.clientWidth);
    await browser.waitUntil(
      async () => ((await boxOf(".slot-right"))?.width ?? 0) >= Math.min(300, wide * 0.2) - 24,
      { timeout: 10_000, timeoutMsg: "the right panel never opened" },
    );
    expect(await $(".slot-right").getAttribute("class")).toContain("arrived");

    // Another view of the right panel: the panel does not arrive again.
    await press(tab(ATTENTION, "Personas"));
    await browser.pause(400);
    expect(await moving(".slot-right")).toEqual([]);
    await press(tab(ATTENTION, "Inbox"));
  });

  it("opens a bottom half from its icon, and only that half arrives", async () => {
    const todos = tab(LOWER_RIGHT, "Todos");
    // A half already open switches its view without arriving: closed first, by its ×.
    const away = '.panel-half[data-side="right"] [aria-label="Put the panel\'s right half away"]';
    if (await $(`${LOWER_RIGHT} [role="tab"][aria-selected="true"]`).isExisting()) {
      await press(away);
      await browser.waitUntil(
        async () => !(await $(`${LOWER_RIGHT} [role="tab"][aria-selected="true"]`).isExisting()),
        { timeout: 10_000, timeoutMsg: "the right half never closed" },
      );
    }
    await press(todos);
    await browser.waitUntil(
      async () =>
        (await $('.panel-half[data-side="right"]').getAttribute("class"))?.includes("arrived") ===
        true,
      { timeout: 10_000, timeoutMsg: "the right half never arrived" },
    );
    expect(await moving('.panel-half[data-side="left"]')).toEqual([]);
  });
});
