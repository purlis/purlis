// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * **The window a scenario starts in**: the default window, 1440×900 (`tauri.conf.json`, ADR 0054's
 * amendment of 2026-10-24), wide enough for the project rail beside both sidebars, so the rail is
 * drawn there as on a person's first launch.
 *
 * The rail hides itself where it would crowd the work (`app/src/railRoom.ts`, #3077): below the
 * narrowest window and the rail together, and, with both sidebars open, below 1280 px and the
 * rail together. A spec that measures a narrow window, or {@link WINDOW_1280}, sets that size
 * itself and measures the rail as the window draws it there.
 */
export const DEFAULT_WINDOW = { width: 1440, height: 900 } as const;

/**
 * **1280×800**: the window a 1280px screen leaves, and the default window before 2026-10-24. The
 * views are still held to fit it with both sidebars open (DS-9's tables, the settings row, the
 * merge list), so the specs that measured the default window measure this one.
 */
export const WINDOW_1280 = { width: 1280, height: 800 } as const;

/** Sets the window's size, and waits until the window is that size and the page has settled at
 *  its width. */
export async function windowAt(size: { width: number; height: number }): Promise<void> {
  await browser.setWindowSize(size.width, size.height);
  let last = -1;
  await browser.waitUntil(
    async () => {
      if ((await browser.getWindowSize()).width !== size.width) return false;
      const now = await browser.execute(() => window.innerWidth);
      const settled = now === last && now <= size.width;
      last = now;
      return settled;
    },
    { timeout: 20_000, interval: 200, timeoutMsg: `the window never settled at ${size.width}px` },
  );
}

/** Puts the window at {@link DEFAULT_WINDOW} unless it is already there. */
export async function theWindowHoldsTheRail(): Promise<void> {
  const now = await browser.getWindowSize();
  if (now.width === DEFAULT_WINDOW.width && now.height === DEFAULT_WINDOW.height) return;
  await windowAt(DEFAULT_WINDOW);
}
