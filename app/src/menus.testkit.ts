// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";

/**
 * **A menu's rows, found as a person finds them: by their name, through the submenus that lead
 * to them** (`docs/ui-primitives.md`, "Menus"). For the tests that drive the window through a
 * menu to get somewhere else; `Menus.test.tsx` and `menuModel.test.ts` own what a menu holds.
 *
 * A row's name is its `aria-label`, its words alone: its text also holds its description, and
 * *Open* is how *Open beside steward 4* starts. Test-only: nothing the window bundles imports
 * this file.
 */

/** The innermost open menu: a submenu is drawn after the menu it opened from. */
export function openMenu(): HTMLElement {
  const menus = screen.getAllByRole("menu");
  return menus[menus.length - 1];
}

/** Every row of a menu, whatever kind of row it is (a toggle is a `menuitemcheckbox`). */
export function menuRowsIn(menu: HTMLElement): HTMLElement[] {
  return Array.from(menu.querySelectorAll<HTMLElement>('[role^="menuitem"]'));
}

/** The names of the innermost open menu's rows, in order. */
export function menuNames(): (string | null)[] {
  return menuRowsIn(openMenu()).map((row) => row.getAttribute("aria-label"));
}

/** The row of the innermost open menu named `name`, or matching it; `undefined` when none is. */
export function findMenuRow(name: string | RegExp): HTMLElement | undefined {
  return menuRowsIn(openMenu()).find((row) => {
    const said = row.getAttribute("aria-label") ?? "";
    return typeof name === "string" ? said === name : name.test(said);
  });
}

/** The row of the innermost open menu named `name`. */
export function menuRow(name: string | RegExp): HTMLElement {
  const row = findMenuRow(name);
  if (row === undefined)
    throw new Error(
      `no row ${String(name)} in the open menu, which has: ${menuNames().join(", ")}`,
    );
  return row;
}

/**
 * Opens the submenus `path` names, each from the one before, and answers the innermost: what
 * `pressMenuPath` does up to its last row, for a test that reads a submenu's rows.
 */
export async function openSubmenus(...path: (string | RegExp)[]): Promise<HTMLElement> {
  for (const name of path) {
    const menus = screen.getAllByRole("menu").length;
    await waitFor(() => menuRow(name));
    await userEvent.click(menuRow(name));
    await waitFor(() => {
      if (screen.getAllByRole("menu").length <= menus)
        throw new Error(`${String(name)} opened no submenu`);
    });
  }
  return openMenu();
}

/**
 * Presses a row of the open menu by its name, through the submenus that lead to it:
 * `pressMenuPath("Open", "Activity")` opens *Open* and presses *Activity* in it.
 */
export async function pressMenuPath(...path: (string | RegExp)[]): Promise<void> {
  const last = path[path.length - 1];
  await openSubmenus(...path.slice(0, -1));
  await waitFor(() => menuRow(last));
  await userEvent.click(menuRow(last));
}
