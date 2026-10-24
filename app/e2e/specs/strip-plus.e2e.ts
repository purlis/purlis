// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { endEveryChat, pressAndStart } from "../opening.js";

/**
 * **The `+` right after the last tab** (DS-7, #1839; the operator's D6): Chrome's `+`, on the
 * chat strip. The workspace strip it was also on is gone (#2916): a workspace is made from the
 * title bar's ⌄ (`title-bar-workspaces.e2e.ts`). With few tabs it sits against
 * the last tab rather than at the far end of the row; with more tabs than fit it is still on
 * screen, after the last tab drawn and before the "N more" button at the right.
 *
 * Where a box is on screen is the whole question and jsdom lays nothing out, so this is asked
 * here, in ranges and never in exact pixels. `Strip.test.tsx`, `WorkspaceLifecycle.test.tsx`
 * and `fits.room.test.tsx` hold the tree and the room arithmetic.
 *
 * It opens its own chats, ends them, and puts the window back as
 * it found it: one app process serves the whole scenario run.
 */

type Box = { left: number; right: number; top: number; bottom: number; width: number };

/** How far past the last tab the `+` may start and still be "right after it": the strip's
 *  own control padding and a rounding pixel, well under one tab's floor. */
const AGAINST = 16;

/** The boxes that say where a strip's `+` is: the strip, its last drawn tab's cell, the `+`
 *  and the show-more button beside the strip, in one pass through the layout. */
async function measured(strip: "Tabs", plus: string) {
  return browser.execute(
    (name: string, label: string) => {
      const box = (el: Element | null | undefined) => {
        if (!el) return null;
        const { left, right, top, bottom, width } = el.getBoundingClientRect();
        return { left, right, top, bottom, width };
      };
      const element = document.querySelector(`[data-strip="${name}"]`);
      const tabs = [...(element?.querySelectorAll('[role="tab"]') ?? [])];
      const last = tabs.at(-1);
      // The tab's cell (its wrapper with the `×` or the gear), which is what the `+` follows.
      const cell = last?.closest(".tab, .workspace") ?? last;
      const row = element?.parentElement?.closest(".bar, .workspaces") ?? null;
      return {
        window: window.innerWidth,
        strip: box(element),
        last: box(cell),
        tabs: tabs.length,
        plus: box(element?.querySelector(`button[aria-label^="${label}"]`)),
        more: box(row?.querySelector('button[aria-label^="Show "]')),
        least: element
          ? Number.parseFloat(getComputedStyle(element).getPropertyValue("--least"))
          : Number.NaN,
        scrolls: element ? element.scrollWidth - element.clientWidth : -1,
      };
    },
    strip,
    plus,
  );
}

type Measured = Awaited<ReturnType<typeof measured>>;

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

/** The `+` is right after the last tab drawn, inside the strip and fully on screen. */
function rightAfterTheLastTab(at: Measured): void {
  expect(at.plus).not.toBeNull();
  expect(at.last).not.toBeNull();
  const plus = at.plus as Box;
  const last = at.last as Box;
  expect(plus.left).toBeGreaterThanOrEqual(last.right - 1);
  expect(plus.left - last.right).toBeLessThanOrEqual(AGAINST);
  expect(inside(plus, at.strip)).toBe(true);
  expect(plus.right).toBeLessThanOrEqual(at.window + 1);
  expect(plus.width).toBeGreaterThan(0);
}

/** With more tabs than fit: the `+` is still drawn, and the "N more" is to its right. */
function beforeTheOverflow(at: Measured): void {
  rightAfterTheLastTab(at);
  expect(at.more).not.toBeNull();
  expect((at.more as Box).left).toBeGreaterThanOrEqual((at.plus as Box).right - 1);
  expect((at.more as Box).right).toBeLessThanOrEqual(at.window + 1);
  // The strip collapses and never scrolls the `+` away (charter-app#130).
  expect(at.scrolls).toBeLessThanOrEqual(1);
}

/**
 * A strip's boxes once its show-more is drawn, read in the same pass that saw it (#1890).
 *
 * Waited for, and bounded: the collapse is the strip's answer to a state the window reaches a
 * render or two after a press, so one read straight after the press may land before it. A
 * strip that never overflows is still a failure here, by its own words.
 */
async function onceItOverflows(strip: "Tabs", plus: string): Promise<Measured> {
  let at = await measured(strip, plus);
  await browser.waitUntil(
    async () => {
      if (at.more === null) at = await measured(strip, plus);
      return at.more !== null;
    },
    { timeout: 30_000, timeoutMsg: `the ${strip} strip never drew its show-more button` },
  );
  return at;
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

const NEW_TAB = "New tab";

describe("the `+` right after the last tab", function () {
  this.timeout(300_000);

  let was = { width: 1280, height: 800 };

  before(async () => {
    was = await browser.getWindowSize();
  });

  after(async () => {
    await windowIs(was.width, was.height);
  });

  describe("on the chat strip", () => {
    after(async () => {
      await endEveryChat();
    });

    it("sits against the last tab, not at the far end of the bar", async () => {
      await pressAndStart(NEW_TAB);
      await browser.waitUntil(async () => (await measured("Tabs", NEW_TAB)).tabs > 0, {
        timeout: 20_000,
        timeoutMsg: "the chat strip never drew a tab",
      });
      const at = await measured("Tabs", NEW_TAB);
      rightAfterTheLastTab(at);
      // The free room falls after the `+`: with the few tabs a spec starts with, it is
      // nowhere near the strip's right end. (Earlier specs end their chats; one that did not
      // would fill the strip, and then this half says nothing.)
      if (at.more === null && at.tabs <= 2)
        expect((at.strip as Box).right - (at.plus as Box).right).toBeGreaterThan(at.least);
    });

    it("with more tabs than fit, stays drawn after the last tab and before N more", async () => {
      await windowIs(1024, was.height);
      // Bounded: a strip that never overflows is a failure here, not a hang.
      for (let opened = 0; opened < 24 && (await measured("Tabs", NEW_TAB)).more === null; opened++)
        await pressAndStart(NEW_TAB);
      beforeTheOverflow(await onceItOverflows("Tabs", NEW_TAB));
    });
  });
});
