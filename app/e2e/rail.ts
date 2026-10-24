// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser } from "@wdio/globals";

/** The project rail's place in the window (#2915). */
const SLOT = '[data-scope="rail"][data-part="slot"]';

/**
 * **Hides the project rail from its own button, or shows it from the title bar's**, and waits
 * until its column has closed or opened (#2915).
 *
 * The specs that measure the work at ADR 0054's narrowest window, or with both sidebars open at
 * the default one, measure it with the rail hidden: the rail is the person's to hide when the
 * work needs its room, and the room those specs hold the work to is the window's without it.
 */
export async function railHidden(hidden: boolean): Promise<void> {
  const isHidden = async () =>
    browser.execute(
      (css: string) => document.querySelector(css)?.hasAttribute("data-hidden") ?? false,
      SLOT,
    );
  if ((await isHidden()) === hidden) return;
  await $(
    hidden ? `${SLOT} [data-rail-toggle="hide"]` : '[data-testid="title-bar"] [data-rail-show]',
  ).click();
  await browser.waitUntil(
    async () =>
      (await isHidden()) === hidden &&
      (await browser.execute(
        (css: string) =>
          Math.round(document.querySelector(css)?.getBoundingClientRect().width ?? -1),
        SLOT,
      )) === (hidden ? 0 : 52),
    { timeout: 10_000, timeoutMsg: `the rail was never ${hidden ? "hidden" : "shown"}` },
  );
}
