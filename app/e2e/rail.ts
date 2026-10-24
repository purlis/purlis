// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * **The window a scenario starts in** (#3077): wide enough for the project rail beside both
 * sidebars, so the rail is drawn, as it is in most windows a person works in.
 *
 * The rail hides itself where it would crowd the work (`app/src/railRoom.ts`): below the
 * narrowest window and the rail together, and, with both sidebars open, below the default
 * window (1280 px, `tauri.conf.json`) and the rail together. A spec that measures a narrow
 * window, or the default one, sets that size itself and measures the rail as the window draws
 * it there.
 */
export const STARTING_WINDOW = { width: 1440, height: 900 } as const;

/** The window the app opens at a person's first launch (`tauri.conf.json`). */
export const DEFAULT_WINDOW = { width: 1280, height: 800 } as const;

/** Sets the window's size, and waits until the page has settled at its width. */
export async function windowAt(size: { width: number; height: number }): Promise<void> {
  await browser.setWindowSize(size.width, size.height);
  let last = -1;
  await browser.waitUntil(
    async () => {
      const now = await browser.execute(() => window.innerWidth);
      const settled = now === last;
      last = now;
      return settled;
    },
    { timeout: 20_000, interval: 200, timeoutMsg: "the window never settled on a width" },
  );
}

/** Puts the window at {@link STARTING_WINDOW} unless it is already there. */
export async function theWindowHoldsTheRail(): Promise<void> {
  const now = await browser.getWindowSize();
  if (now.width === STARTING_WINDOW.width && now.height === STARTING_WINDOW.height) return;
  await windowAt(STARTING_WINDOW);
}
