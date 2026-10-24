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

/**
 * **Starts recording the animations that start on what `css` names**, by their keyframes' name.
 * An arrival's mark comes off when its animation ends (D-2917-20), so a spec that polled for the
 * mark could miss it; the event is heard as it happens, from the page. {@link started} reads what
 * was heard since.
 */
async function hear(css: string): Promise<void> {
  await browser.execute((selector: string) => {
    const kept = window as unknown as { heard2917?: Record<string, string[]> };
    const heard = (kept.heard2917 ??= {});
    heard[selector] = [];
    const listen = (e: AnimationEvent) => {
      if (e.target instanceof Element && e.target.matches(selector)) {
        heard[selector]?.push(e.animationName);
      }
    };
    document.addEventListener("animationstart", listen, true);
  }, css);
}

/** The animations heard starting on what `css` names since {@link hear} was called for it. */
async function started(css: string): Promise<string[]> {
  return browser.execute(
    (selector: string) =>
      (window as unknown as { heard2917?: Record<string, string[]> }).heard2917?.[selector] ?? [],
    css,
  );
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

    await hear(".slot-right");
    await press(tab(ATTENTION, "Inbox"));
    // 300px where the row of regions has room, else its 20% of that row (R124): the row is the
    // window less the project rail, the bars and the gutters.
    const row = (await boxOf(".region-row"))?.width ?? 0;
    await browser.waitUntil(
      async () => ((await boxOf(".slot-right"))?.width ?? 0) >= Math.min(300, row * 0.2) - 24,
      { timeout: 10_000, timeoutMsg: "the right panel never opened" },
    );
    await browser.waitUntil(async () => (await started(".slot-right")).includes("purlis-open"), {
      timeout: 10_000,
      timeoutMsg: "the right panel never arrived",
    });

    // Another view of the right panel: the panel does not arrive again.
    await hear(".slot-right");
    await press(tab(ATTENTION, "Personas"));
    await browser.pause(400);
    expect(await started(".slot-right")).toEqual([]);
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
    await hear('.panel-half[data-side="right"]');
    await hear('.panel-half[data-side="left"]');
    await press(todos);
    await browser.waitUntil(
      async () => (await started('.panel-half[data-side="right"]')).includes("purlis-reveal"),
      { timeout: 10_000, timeoutMsg: "the right half never arrived" },
    );
    expect(await started('.panel-half[data-side="left"]')).toEqual([]);
    expect(await moving('.panel-half[data-side="left"]')).toEqual([]);
  });
});
