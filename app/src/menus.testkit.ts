// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
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

/**
 * **Opens chat `name`'s one list** (#4753): a right-click on its row in the Chats list, the
 * list its pane's ⋯ and the palette have too, and answers the menu once it is up. A chat with no
 * tab has it whole, and a tab's own menu has none of a chat's rows.
 */
export async function chatList(name: string): Promise<HTMLElement> {
  const tree = await screen.findByRole("tree", { name: "Chats of this project" });
  const row = await waitFor(() => {
    const found = Array.from(tree.querySelectorAll<HTMLElement>('[role="treeitem"]')).find(
      (one) => one.querySelector(".session")?.textContent === name,
    );
    if (found === undefined) throw new Error(`no row is named ${name}`);
    return found;
  });
  fireEvent.contextMenu(row);
  return screen.findByRole("menu");
}

/**
 * **Asks to end chat `name`** the one way there is (#4753, #4756): End… on its one list. A close
 * ends nothing. Answers the question End… asks, once it is up.
 */
export async function askToEnd(name: string): Promise<HTMLElement> {
  // Its pane's ⋯ where it is on screen, and its row's menu where it is not.
  const more = screen.queryAllByRole("button", { name: `More for ${name}` });
  if (more.length > 0) await userEvent.click(more[0]);
  const list = more.length > 0 ? await screen.findByRole("menu") : await chatList(name);
  await userEvent.click(within(list).getByRole("menuitem", { name: "End…" }));
  return screen.findByRole("alertdialog");
}
