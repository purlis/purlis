// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * **The region keys as the window hears them** (F6 and Shift+F6, #1741 P-13; train 78's e2e
 * lessons), shared by `panel-toggle.e2e.ts` and `keyboard-journey.e2e.ts`: a bare key through the
 * driver first, and where nothing came of it within a few seconds, the same key dispatched from
 * the page to the element that has the keyboard, which the window's capture-phase listener hears;
 * a chord from the page alone, since the driver does not hold its modifier.
 */

/** The region frame, which names its F6 cycle in `data-cycle`. */
export const FRAME = ".region-frame[data-cycle]";

/** Takes the keyboard off whatever has it, so no chat keeps the next key. */
export async function keyboardToThePage(): Promise<void> {
  await browser.execute(() => {
    const on = document.activeElement;
    if (on instanceof HTMLElement) on.blur();
  });
}

/** Dispatches `init` as a `keydown` from the page to the element that has the keyboard. */
export async function keyFromThePage(init: KeyboardEventInit): Promise<void> {
  await browser.execute((given: KeyboardEventInit) => {
    (document.activeElement ?? document.body).dispatchEvent(
      new KeyboardEvent("keydown", { ...given, bubbles: true, cancelable: true }),
    );
  }, init);
}

/**
 * Presses a key as the window hears it. A chord is dispatched from the page: the driver does not
 * hold a modifier across `browser.keys` (`palette.e2e.ts`), so its Shift+F6 would arrive as a
 * bare F6 and step the wrong way. A bare key goes through the driver first and, where `landed`
 * has not come true within a few seconds, from the page.
 */
export async function keyWithFallback(
  keys: string[] | "chord",
  init: KeyboardEventInit,
  landed: () => Promise<boolean>,
): Promise<void> {
  if (keys === "chord") return keyFromThePage(init);
  await browser.keys(keys);
  const came = await browser
    .waitUntil(landed, { timeout: 3_000, interval: 150 })
    .then(() => true)
    .catch(() => false);
  if (!came) await keyFromThePage(init);
}

/**
 * Where each region F6 stops in is drawn, by the stop's name in the frame's cycle: the frame's
 * own `REGION_OF` (`RegionFrame.tsx`), read here and never imported, since a spec runs outside
 * the app's bundle.
 */
export const REGION_OF: Record<string, string> = {
  left: ".slot-left .region-view:not([hidden])",
  centre: ".region-centre",
  right: ".slot-right .region-view:not([hidden])",
  "panel-left": '.panel-half[data-side="left"]:not([hidden])',
  "panel-right": '.panel-half[data-side="right"]:not([hidden])',
};

/** The stop the keyboard is in, by its name in the cycle; `""` where it is in none. */
export async function regionWithTheKeyboard(): Promise<string> {
  return browser.execute(
    (frame: string, regions: Record<string, string>) => {
      const root = document.querySelector(frame);
      const on = document.activeElement;
      if (root === null || on === null) return "";
      return (
        Object.entries(regions).find(([, selector]) =>
          [...root.querySelectorAll(selector)].some((region) => region.contains(on)),
        )?.[0] ?? ""
      );
    },
    FRAME,
    REGION_OF,
  );
}

/** F6, or Shift+F6 with `back`, until the keyboard is in a region other than `from`. */
export async function f6From(from: string, back = false): Promise<string> {
  await keyWithFallback(
    back ? "chord" : ["F6"],
    { key: "F6", code: "F6", shiftKey: back },
    async () => {
      const now = await regionWithTheKeyboard();
      return now !== "" && now !== from;
    },
  );
  let now = "";
  await browser.waitUntil(
    async () => {
      now = await regionWithTheKeyboard();
      return now !== "" && now !== from;
    },
    {
      timeout: 10_000,
      timeoutMsg: `${back ? "Shift+F6" : "F6"} did not move the keyboard out of ${from || "the page"}`,
    },
  );
  return now;
}

/** How many stops the frame's cycle has: the most F6 presses that walk it once. */
export async function stopsInTheCycle(): Promise<number> {
  return browser.execute(
    (frame: string) =>
      (document.querySelector(frame)?.getAttribute("data-cycle") ?? "").split(" ").filter(Boolean)
        .length,
    FRAME,
  );
}
