// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { endEveryChat, pressAndStart } from "../opening.js";

/**
 * **The chat strip is the editor's tab bar** (DS-6, #1836; ADR 0036 as amended 2026-10-12):
 * it sits at the top of the centre column, directly above the chat it shows, while the
 * workspaces are the title bar's (#2916).
 *
 * Where a box is on screen is the whole question, and jsdom lays nothing out, so this is asked
 * here, in ranges and never in exact pixels: a box inside another within a pixel of rounding.
 * `FourRegions.test.tsx` holds the tree that puts it there.
 *
 * And at the narrowest window purlis supports (1024 px, ADR 0054) with both sidebars open, the
 * strip is still usable: the selected tab is drawn, and once there are more tabs than fit, the
 * show-more button and the `+` are on screen beside it, inside the centre column.
 *
 * It opens its own tabs and ends every chat it opened, and puts the window back as it found it:
 * one app process serves the whole scenario run.
 */

type Box = { left: number; right: number; top: number; bottom: number; width: number };

/** Where the strip, its parts and the regions around it are, in one pass through the layout. */
async function measured() {
  return browser.execute(() => {
    const boxOf = (selector: string) => {
      const el = document.querySelector(selector);
      if (el === null) return null;
      const { left, right, top, bottom, width } = el.getBoundingClientRect();
      return { left, right, top, bottom, width };
    };
    const strip = document.querySelector('[data-strip="Tabs"]');
    const selected = strip?.querySelector('[role="tab"][aria-selected="true"]');
    const cell = selected?.closest(".tab") ?? selected;
    const box = cell?.getBoundingClientRect();
    return {
      window: window.innerWidth,
      centre: boxOf('[data-panel][id="region-centre"]'),
      left: boxOf('[data-panel][id="region-left"]'),
      right: boxOf('[data-panel][id="region-right"]'),
      bar: boxOf('[data-panel][id="region-centre"] header.bar'),
      panes: boxOf('[data-panel][id="region-centre"] .panes'),
      selected: box ? { left: box.left, right: box.right, width: box.width } : null,
      least: strip ? Number.parseFloat(getComputedStyle(strip).getPropertyValue("--least")) : NaN,
      more: boxOf('header.bar button[aria-label^="Show "]'),
      adding: boxOf('header.bar button[aria-label="New tab"]'),
      scrolls: strip ? strip.scrollWidth - strip.clientWidth : -1,
    };
  });
}

/** `inner` lies inside `outer`, a pixel of rounding either way. */
function inside(inner: Box | null, outer: Box | null): boolean {
  if (inner === null || outer === null) return false;
  return (
    inner.left >= outer.left - 1 &&
    inner.right <= outer.right + 1 &&
    inner.top >= outer.top - 1 &&
    inner.bottom <= outer.bottom + 1
  );
}

/** The two sidebars: each one's activity bar, and the slot it opens. */
const SIDES = [
  ["Navigation", "region-left"],
  ["Attention", "region-right"],
] as const;

/**
 * Opens each sidebar that is put away, by a press of a view's icon on its activity bar (the open
 * one's, else the first), so the narrow case does not depend on what a spec run before it left.
 * Returns each icon it pressed, as `bar\u0000name`, for {@link putAwayAgain}.
 */
async function bothSidesOpen(): Promise<string[]> {
  const pressed: string[] = [];
  for (const [bar, slot] of SIDES) {
    const side = await $(`[data-panel][id="${slot}"]`);
    if (((await side.getSize("width")) as number) > 0) continue;
    const icons = `[role="tablist"][aria-label="${bar}"] [role="tab"]`;
    const open = await $(`${icons}[aria-selected="true"]`);
    const icon = (await open.isExisting()) ? open : await $(icons);
    const name = (await icon.getAttribute("aria-label")) ?? "";
    await icon.click();
    await browser.waitUntil(async () => ((await side.getSize("width")) as number) > 0, {
      timeout: 20_000,
      timeoutMsg: `the ${bar} side did not open when its view was pressed`,
    });
    pressed.push(`${bar}\u0000${name}`);
  }
  return pressed;
}

/** Puts away again each sidebar {@link bothSidesOpen} opened: a press of its open view. */
async function putAwayAgain(pressed: string[]): Promise<void> {
  for (const one of pressed) {
    const [bar, name] = one.split("\u0000");
    await (
      await $(`[role="tablist"][aria-label="${bar}"] [role="tab"][aria-label="${name}"]`)
    ).click();
  }
}

/** Sizes the window and waits until its width stops changing. */
async function windowIs(width: number, height: number): Promise<void> {
  await browser.setWindowSize(width, height);
  let last = -1;
  await browser.waitUntil(
    async () => {
      const now = await browser.execute(() => window.innerWidth);
      const settled = now === last;
      last = now;
      return settled;
    },
    { timeout: 20_000, interval: 250, timeoutMsg: "the window never settled on a width" },
  );
}

describe("the chat strip in the centre column", () => {
  let was = { width: 1280, height: 800 };

  before(async () => {
    was = await browser.getWindowSize();
    await pressAndStart("New tab");
    await browser.waitUntil(async () => (await measured()).selected !== null, {
      timeout: 20_000,
      timeoutMsg: "the chat strip never drew a selected tab",
    });
  });

  after(async () => {
    await windowIs(was.width, was.height);
    await endEveryChat();
  });

  it("sits at the top of the centre column, above the panes and beside the sidebars", async () => {
    const at = await measured();
    expect(at.bar).not.toBeNull();
    expect(inside(at.bar, at.centre)).toBe(true);
    // At the top of the column, and the panes start where it ends.
    expect(Math.abs((at.bar as Box).top - (at.centre as Box).top)).toBeLessThanOrEqual(2);
    expect(Math.abs((at.panes as Box).top - (at.bar as Box).bottom)).toBeLessThanOrEqual(2);
    // Across the column and no further: not over either sidebar.
    expect(Math.abs((at.bar as Box).left - (at.centre as Box).left)).toBeLessThanOrEqual(2);
    expect(Math.abs((at.bar as Box).right - (at.centre as Box).right)).toBeLessThanOrEqual(2);
    if (at.left && at.left.width > 0)
      expect((at.bar as Box).left).toBeGreaterThanOrEqual(at.left.right - 1);
    if (at.right && at.right.width > 0)
      expect((at.bar as Box).right).toBeLessThanOrEqual(at.right.left + 1);
  });

  it("keeps the chat strip at the chrome height, and grows no strip past a control", async () => {
    // DS-1's control baseline (ADR 0094) gives an unclassed control the 28px control height
    // by padding; a strip's own rules set their buttons' padding, so no strip may grow by it.
    // Ranges, not pixels, read from the core tokens themselves: the chat strip, the editor's
    // tab bar, is at least the chrome height and no taller than a docked tab, a control and 3px
    // (#2914, the switcher prototype's 31px tabs). The projects are the rail now (#2915), a
    // column and not a row. The workspace row is not moved to the chrome height yet (its own
    // `min-height` is 2rem), so it is held to the larger of one control height and that, and a
    // pixel and a half of rounding: Purlis's control is 26px, under the row's 2rem, and its
    // words are set at a taller line height (#3034).
    const at = await browser.execute(() => {
      const probe = (token: string) => {
        const el = document.createElement("div");
        el.style.height = `var(${token})`;
        document.body.append(el);
        const height = el.getBoundingClientRect().height;
        el.remove();
        return height;
      };
      const heightOf = (selector: string) =>
        document.querySelector(selector)?.getBoundingClientRect().height ?? Number.NaN;
      const row = document.createElement("div");
      row.style.height = "2rem";
      document.body.append(row);
      const rowHeight = row.getBoundingClientRect().height;
      row.remove();
      return {
        chrome: probe("--size-chrome"),
        control: probe("--size-control"),
        tab: (() => {
          const el = document.createElement("div");
          el.style.height = "calc(var(--size-control) + calc(3rem / 14))";
          document.body.append(el);
          const height = el.getBoundingClientRect().height;
          el.remove();
          return height;
        })(),
        row: rowHeight,
        chats: heightOf("header.bar"),
      };
    });
    expect(at.chrome).toBeGreaterThan(0);
    expect(at.control).toBeGreaterThan(at.chrome);
    expect(at.chats).toBeGreaterThanOrEqual(at.chrome - 1);
    // Each strip over its bound, named with its height, so a red run says which one grew.
    const over = [["the chat strip", at.chats, at.tab + 0.5]].flatMap(([what, height, most]) =>
      Number(height) <= Number(most) ? [] : [`${what} is ${height}px, over ${most}px`],
    );
    expect(over).toEqual([]);
  });

  it("at 1024 px with both sidebars open, still draws the selected tab, show-more and +", async () => {
    // Both sidebars open, whatever the specs before this one left, and put back after.
    const opened = await bothSidesOpen();
    try {
      await atTheNarrowest();
    } finally {
      await putAwayAgain(opened);
    }
  });

  async function atTheNarrowest() {
    await windowIs(1024, was.height);
    const narrow = await measured();
    expect(narrow.window).toBeLessThanOrEqual(1024);
    // Both sidebars are open: this is the narrowest the centre is by default.
    expect(narrow.left?.width ?? 0).toBeGreaterThan(0);
    expect(narrow.right?.width ?? 0).toBeGreaterThan(0);

    // More tabs than the strip has room for, so the strip has to collapse. Bounded: a strip that
    // never overflows is a failure here, not a hang.
    for (let opened = 0; opened < 16 && (await measured()).more === null; opened++)
      await pressAndStart("New tab");

    const at = await measured();
    // The strip overflowed into its show-more button.
    expect(at.more).not.toBeNull();
    // The selected tab is drawn, whole: no narrower than the floor the strip fits by.
    expect(at.selected).not.toBeNull();
    expect((at.selected as { width: number }).width).toBeGreaterThanOrEqual(at.least - 1);
    // Everything the strip draws is inside the centre column, and the strip does not scroll.
    expect(inside(at.bar, at.centre)).toBe(true);
    expect(inside(at.more, at.bar)).toBe(true);
    expect(inside(at.adding, at.bar)).toBe(true);
    const sel = at.selected as { left: number; right: number };
    expect(sel.left).toBeGreaterThanOrEqual((at.bar as Box).left - 1);
    expect(sel.right).toBeLessThanOrEqual((at.more as Box).left + 1);
    expect(at.scrolls).toBeLessThanOrEqual(1);
  }
});
