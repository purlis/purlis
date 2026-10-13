// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { SAID, SUBMENU } from "../src/menuWords.ts";

/**
 * **A context menu's rows, by the words the window itself draws them with.**
 *
 * A spec never writes a row's words down: it names the row (`ROW.rename`, `SUB.open`), and the
 * words are `src/menuWords.ts`'s, the one place the window reads them from. A row renamed there
 * is renamed here, and no spec is edited.
 */
export const ROW = SAID;

/** The rows that open a submenu, by their words. */
export const SUB = {
  open: SUBMENU.open.said,
  ask: SUBMENU.ask.said,
  stop: SUBMENU.stop.said,
  curate: SUBMENU.curate.said,
  moveTo: SUBMENU.moveTo.said,
} as const;

/**
 * Presses a row of the open menu **by its name**, through the submenus that lead to it:
 * `pressMenuPath(SUB.open, ROW.activity)` opens *Open* and presses *Activity* in it.
 *
 * By name and exactly, never by how the row's text starts: a row's text also holds its
 * description, and *Open* is how *Open beside steward 4* starts. Each row is pressed by its own
 * click from the page, since a submenu can sit past the window's edge where the driver counts a
 * row as not on screen. A row that opens a submenu is waited on until that submenu is up.
 */
export async function pressMenuPath(...path: string[]): Promise<void> {
  for (const [at, name] of path.entries()) {
    const menus = await browser.execute(() => document.querySelectorAll('[role="menu"]').length);
    await browser.waitUntil(
      async () =>
        browser.execute((wanted: string) => {
          const open = document.querySelectorAll('[role="menu"]');
          const row = [
            ...(open[open.length - 1]?.querySelectorAll<HTMLElement>('[role^="menuitem"]') ?? []),
          ].find((one) => one.getAttribute("aria-label") === wanted);
          if (row === undefined || row.getAttribute("aria-disabled") === "true") return false;
          row.click();
          return true;
        }, name),
      {
        timeout: 10_000,
        interval: 200,
        timeoutMsg: `no open menu has a row named ${JSON.stringify(name)}`,
      },
    );
    if (at === path.length - 1) return;
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => document.querySelectorAll('[role="menu"]').length)) > menus,
      { timeout: 10_000, timeoutMsg: `${JSON.stringify(name)} opened no submenu` },
    );
  }
}

/** The names of the rows of every open menu, the innermost menu last, whatever kind of row. */
export async function menuNames(): Promise<string[][]> {
  return browser.execute(() =>
    [...document.querySelectorAll('[role="menu"]')].map((menu) =>
      [...menu.querySelectorAll('[role^="menuitem"]')].map(
        (row) => row.getAttribute("aria-label") ?? (row.textContent ?? "").trim(),
      ),
    ),
  );
}
