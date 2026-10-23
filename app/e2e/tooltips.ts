// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * **What a hint says, read as the keyboard reads it** (#2908): the window's hints are the shared
 * tooltip (`app/src/ui/tooltip`), drawn only while it is open, where they were a `title` a spec
 * could read off the element. The keyboard's focus opens a tooltip at once, so each element
 * matching `selector` is focused as the keyboard would focus it, the tooltip it opens is read,
 * and it is let go again before the next, all in one pass inside the page.
 *
 * `null` for an element that shows no tooltip: it has no hint, or it takes no keyboard (a mark
 * inside a row, whose row says what it means).
 */
export async function tooltipsOf(selector: string): Promise<(string | null)[]> {
  return browser.execute(async (within: string) => {
    const HINT = '[role="tooltip"][data-scope="tooltip"]';
    const open = () => document.querySelector<HTMLElement>(`${HINT}:not([data-exiting])`);
    const until = async (done: () => boolean) => {
      for (let tries = 0; tries < 100 && !done(); tries++)
        await new Promise((resolve) => setTimeout(resolve, 20));
    };
    const said: (string | null)[] = [];
    for (const element of document.querySelectorAll<HTMLElement>(within)) {
      // The last one gone altogether, its leaving drawn too, so what is read is this one's.
      await until(() => document.querySelector(HINT) === null);
      element.scrollIntoView({ block: "nearest" });
      // The keyboard is what moves the focus now, as React Aria tells it from the pointer.
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true }));
      document.dispatchEvent(new KeyboardEvent("keyup", { key: "Tab", bubbles: true }));
      if (document.activeElement === element) element.blur();
      element.focus();
      await until(() => open() !== null);
      said.push(document.activeElement === element ? (open()?.textContent ?? null) : null);
      element.blur();
    }
    await until(() => document.querySelector(HINT) === null);
    return said;
  }, selector);
}

/** What the first element matching `selector` says as its hint, or `null`. */
export async function tooltipOf(selector: string): Promise<string | null> {
  return (await tooltipsOf(selector))[0] ?? null;
}

/**
 * **Escape, until what `isOpen` reads is gone**, three at most: the first Esc may be a tooltip's,
 * which the keyboard's focus opened (WCAG 1.4.13, the tooltip's alone), and the next is the
 * dialog's or the menu's.
 */
export async function escapeUntilClosed(isOpen: () => Promise<boolean>): Promise<void> {
  for (let left = 0; left < 3 && (await isOpen()); left++) {
    await browser.keys(["Escape"]);
    await browser.pause(200);
  }
}
