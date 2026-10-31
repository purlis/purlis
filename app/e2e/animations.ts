// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { listenForAnimations, type WithHeardAnimations } from "./heardAnimations.js";

/** What {@link hearAnimations} answers. */
export type Heard = {
  /** The animations heard starting on `css` so far, by their keyframes' names. */
  on: (css: string) => Promise<string[]>;
  /** Waits for `name` to be heard on `css`. With motion off there is nothing to wait for. */
  until: (css: string, name: string) => Promise<void>;
};

/**
 * **Hears the animations that start on what `selectors` name, from before an action** (#3101):
 * call it, act, then ask. A part's arrival mark comes off when its animation ends, so a spec that
 * polled for the mark, or for a running animation, missed it on a fast machine. An event cannot
 * be missed once the listener is on.
 *
 * With reduced motion on it hears nothing ({@link listenForAnimations}). A later call replaces
 * the listener of the one before.
 */
export async function hearAnimations(...selectors: string[]): Promise<Heard> {
  await browser.execute(listenForAnimations, selectors);
  const on = (css: string) =>
    browser.execute(
      (selector: string) =>
        (window as unknown as WithHeardAnimations).heardAnimations?.heard[selector] ?? [],
      css,
    );
  return {
    on,
    until: async (css, name) => {
      const off = await browser.execute(
        () => document.documentElement.getAttribute("data-motion") === "reduced",
      );
      if (off) return;
      await browser.waitUntil(async () => (await on(css)).includes(name), {
        timeout: 10_000,
        timeoutMsg: `${name} never started on ${css}`,
      });
    },
  };
}
