// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * **The room two panes side by side need**, in CSS pixels: twice `PANE_FLOOR.width`
 * (`src/paneDrop.ts`). Below it the window folds a split's other side to a rail and greys out
 * Split right (`src/paneFold.ts`, #4766). A few pixels more are added for the separator. Written
 * here, not imported, because `paneDrop.ts` brings the whole tab model into the runner.
 */
export const TWO_PANES = 2 * 300 + 8;

/**
 * **Widens the window until its panes have room for two side by side** (#4766), and gives back
 * the width it ended at. A spec that wants "the narrowest window, with a split" asks for 1024 px
 * and then this. What 1024 px leaves the panes is not fixed: it depends on what the sidebar and
 * the Panel hold when the spec runs. On the macOS runners it was under 600 px, so the second pane
 * folded to a rail and Split right was refused (merge-group runs 41739877099 and 41746176864),
 * while Linux had room. Only the shortfall is added, so the panes stay as narrow as a split
 * allows. A window that is wide enough already is left as it is.
 */
export async function roomForTwoPanes(): Promise<number> {
  for (let tries = 0; tries < 4; tries++) {
    const [room, inner] = await browser.execute(() => {
      // The front project's panes: the one that is laid out, not a hidden project behind it.
      const shown = [...document.querySelectorAll<HTMLElement>(".panes")].find(
        (one) => one.getBoundingClientRect().width > 0,
      );
      return [shown?.getBoundingClientRect().width ?? 0, window.innerWidth];
    });
    if (room === 0 || room >= TWO_PANES) return inner;
    const size = await browser.getWindowSize();
    await browser.setWindowSize(size.width + Math.ceil(TWO_PANES - room), size.height);
    let last = -1;
    await browser.waitUntil(
      async () => {
        const now = await browser.execute(() => window.innerWidth);
        const settled = now === last;
        last = now;
        return settled;
      },
      { timeout: 20_000, interval: 250, timeoutMsg: "the window never stopped resizing" },
    );
  }
  return browser.execute(() => window.innerWidth);
}
