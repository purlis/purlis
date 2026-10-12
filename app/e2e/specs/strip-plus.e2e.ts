// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { realpathSync, renameSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { endEveryChat, pressAndStart } from "../opening.js";

/**
 * **The `+` right after the last tab, in both strips** (DS-7, #1839; the operator's D6):
 * Chrome's `+`, on the workspace strip and on the chat strip. With few tabs it sits against
 * the last tab rather than at the far end of the row; with more tabs than fit it is still on
 * screen, after the last tab drawn and before the "N more" button at the right.
 *
 * Where a box is on screen is the whole question and jsdom lays nothing out, so this is asked
 * here, in ranges and never in exact pixels. `Strip.test.tsx`, `WorkspaceLifecycle.test.tsx`
 * and `fits.room.test.tsx` hold the tree and the room arithmetic.
 *
 * It opens its own chats and its own project, ends and closes them, and puts the window back as
 * it found it: one app process serves the whole scenario run.
 */

type Box = { left: number; right: number; top: number; bottom: number; width: number };

/** How far past the last tab the `+` may start and still be "right after it": the strip's
 *  own control padding and a rounding pixel, well under one tab's floor. */
const AGAINST = 16;

/** The boxes that say where a strip's `+` is: the strip, its last drawn tab's cell, the `+`
 *  and the show-more button beside the strip, in one pass through the layout. */
async function measured(strip: "Tabs" | "Workspaces", plus: string) {
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

/** What the app answered a command with, insisting it answered at all. */
async function ask<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const answer = await browser.executeAsync(
    (
      name: string,
      passed: Record<string, unknown>,
      done: (out: { ok?: unknown; trouble?: string }) => void,
    ) => {
      void window.__TAURI__.core
        .invoke(name, passed)
        .then((ok) => done({ ok }))
        .catch((e: unknown) => done({ trouble: String(e) }));
    },
    command,
    args,
  );
  if (answer.trouble !== undefined) throw new Error(`${command} refused: ${answer.trouble}`);
  return answer.ok as T;
}

const NEW_TAB = "New tab";
const NEW_WORKSPACE = "New workspace";

describe("the `+` right after the last tab", function () {
  // Making a dozen workspaces writes a dozen files each.
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
      beforeTheOverflow(await measured("Tabs", NEW_TAB));
    });
  });

  describe("on the workspace strip", () => {
    /** The project the launch opened, which this part never touches. */
    let first = "";
    /** This part's own project: the fixture plane, so its workspaces are its own to make. */
    const mine = (() => {
      const copied = copyFixturePlane();
      const renamed = join(dirname(copied), "strip-plus");
      renameSync(copied, renamed);
      const root = realpathSync(renamed);
      anEmptyRecord(root);
      return root;
    })();
    const plusOf = `[data-strip="Workspaces"] .strip-doing button[aria-label^="${NEW_WORKSPACE}"]`;

    /** How many workspaces the strip knows of: the ones it draws and the ones behind N more. */
    async function known(): Promise<number> {
      return browser.execute(() => {
        const strip = document.querySelector('[data-strip="Workspaces"]');
        const drawn = strip?.querySelectorAll('[role="tab"]').length ?? 0;
        const more = strip?.parentElement?.querySelector('button[aria-label^="Show "]');
        const hidden = Number.parseInt(
          /\d+/.exec(more?.getAttribute("aria-label") ?? "")?.[0] ?? "0",
        );
        return drawn + hidden;
      });
    }

    before(async () => {
      first = (await ask<string[]>("open_planes"))[0];
      // Through the opener, as `workspace-lifecycle.e2e.ts` does and for its reason: a window
      // learns it holds a project by opening one.
      await $('[data-strip="Projects"] button[aria-label="Open a project…"]').click();
      const box = await $('[data-setting="open-by-path"] input');
      await box.waitForDisplayed({ timeout: 20_000 });
      await box.addValue(mine);
      await $('form.by-path button[type="submit"]').click();
      const question = await $('[role="dialog"]');
      await question.waitForDisplayed({ timeout: 30_000 });
      await $('[role="dialog"]').$("button=Open project").click();
      await browser.waitUntil(async () => (await ask<string[]>("open_planes")).includes(mine), {
        timeout: 30_000,
        timeoutMsg: "this spec's project never opened",
      });
      await $(plusOf).waitForDisplayed({ timeout: 30_000 });
    });

    after(async () => {
      for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first)) {
        await ask("close_plane", { plane });
      }
    });

    it("sits against the last workspace, not at the far end of the row", async () => {
      await windowIs(was.width, was.height);
      const at = await measured("Workspaces", NEW_WORKSPACE);
      rightAfterTheLastTab(at);
      expect((at.strip as Box).right - (at.plus as Box).right).toBeGreaterThan(at.least);
    });

    it("with more workspaces than fit, stays drawn after the last and before N more", async () => {
      await windowIs(1024, was.height);
      // Each made from the window's own `+`, which pins it, so it is on the strip (ADR 0054).
      for (
        let made = 0;
        made < 16 && (await measured("Workspaces", NEW_WORKSPACE)).more === null;
        made++
      ) {
        const before = await known();
        await $(plusOf).click();
        const name = await $('[role="dialog"] input');
        await name.waitForDisplayed({ timeout: 20_000 });
        await name.setValue(`extra${String(made).padStart(2, "0")}`);
        await $("button=Create workspace").click();
        await browser.waitUntil(async () => (await known()) > before, {
          timeout: 60_000,
          timeoutMsg: "a workspace made from the `+` never reached the strip",
        });
      }
      beforeTheOverflow(await measured("Workspaces", NEW_WORKSPACE));
    });
  });
});
