// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";

/**
 * **About's licences, as a view tab of the built app** (#1719, #1784).
 *
 * `LicencesView.window.test.tsx` and `About.test.tsx` hold what jsdom settles, against a stand-in
 * for the third-party notices: the dev server and vitest serve none. What only a build has is
 * here: `vite build` generated the notices from the locked dependency graph
 * (`tools/third-party-notices.mjs`), so the tab lists the crates and npm packages this very build
 * was made from, under purlis's own NOTICE and licence.
 *
 * **It leaves the window as it found it**: the tab it opens it closes.
 */

const TABS = '[data-strip="Tabs"]';
const TITLE = "Licences";

/** The names on the tab strip, left to right. */
async function tabNames(): Promise<string[]> {
  return browser.execute(() =>
    [
      ...(document.querySelector('[data-strip="Tabs"]')?.querySelectorAll('[role="tab"]') ?? []),
    ].map((tab) => tab.textContent ?? ""),
  );
}

describe("About's licences", () => {
  it("open as a view tab listing purlis's NOTICE first, then every third-party crate and npm package", async () => {
    const about = await $('[data-testid="title-about"]');
    await about.waitForDisplayed({ timeout: 30_000 });
    await $(TABS).waitForExist({ timeout: 30_000 });
    try {
      await about.click();
      const dialog = await $('[role="dialog"][aria-describedby="about-what"]');
      await dialog.waitForDisplayed({ timeout: 20_000 });
      await (await dialog.$("a=their licences")).click();
      // A project is in front, so it takes the ask and About closes.
      await browser.waitUntil(async () => !(await dialog.isExisting()), {
        timeout: 20_000,
        timeoutMsg: "About did not close once the licences went to a tab",
      });
      await browser.waitUntil(async () => (await tabNames()).some((name) => name.includes(TITLE)), {
        timeout: 20_000,
        timeoutMsg: "no Licences tab opened",
      });

      const view = await $('[data-testid="licences-view"]');
      await view.waitForExist({ timeout: 20_000 });
      const third = await view.$('[data-testid="third-party-notices"]');
      // The lazy chunk is read when the view is drawn.
      await third.waitForExist({ timeout: 20_000 });

      const order = await browser.execute(() => {
        const view = document.querySelector('[data-testid="licences-view"]');
        const own = view?.querySelector('[data-testid="own-licence"]');
        const notices = view?.querySelector('[data-testid="third-party-notices"]');
        if (!own || !notices) return null;
        return {
          ownFirst: Boolean(
            own.compareDocumentPosition(notices) & Node.DOCUMENT_POSITION_FOLLOWING,
          ),
          own: own.textContent ?? "",
          head: notices.querySelector("p")?.textContent ?? "",
          summaries: [...notices.querySelectorAll("summary")].map((s) => s.textContent ?? ""),
        };
      });
      if (order === null) throw new Error("the Licences view has no own licence or no notices");
      expect(order.ownFirst).toBe(true);
      expect(order.own).toMatch(/Copyright 2026 Purlis[\s\S]*Version 2\.0, January 2004/);
      // Counted from this build's own graph, so a number rather than a pin on one.
      expect(order.head).toMatch(/built with the \d+ Rust crates and \d+ npm packages below/);
      // The app's own framework and window library, each under the text it ships.
      expect(order.summaries.some((s) => /(^|, )tauri \d+\.\d+\.\d+ \(/.test(s))).toBe(true);
      expect(order.summaries.some((s) => /(^|, )react \d+\.\d+\.\d+ \(/.test(s))).toBe(true);
    } finally {
      if ((await tabNames()).some((name) => name.includes(TITLE))) {
        await $(`${TABS} button[aria-label="Close ${TITLE}"]`).click();
        await browser.waitUntil(
          async () => !(await tabNames()).some((name) => name.includes(TITLE)),
          { timeout: 20_000, timeoutMsg: "the Licences tab did not close" },
        );
      }
    }
  });
});
