// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync, readdirSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  closeMenus,
  closeTab,
  focusWorkspace,
  menuRows,
  openMenuOn,
  press,
  pressMenuRow,
  projectRoot,
  tabNames,
  textOf,
  untilSays,
} from "../acting.js";
import { showView } from "../opening.js";

/**
 * **A memory moved to another store from its row's menu, and its Undo** (KN-3, #1190), in the
 * built app against the real core: the workspace's Memory section lists `alpha`'s journal, a
 * row's menu has *Move to ▸* with a row per store but its own, the pick moves the file whole
 * with nothing asked first, and the line it leaves has Undo, which puts the file back **under
 * the exact name it had**, a journal's name to the second (`restore_as`, 2f1eb2a3).
 *
 * The row's menu, never the palette's row: the palette's catalogue is another lane's this
 * round. The menu is opened with the `contextmenu` a right-click sends (`acting.ts`).
 *
 * **It leaves the project as it found it**: should Undo not land, the file and both stores'
 * indexes are put back by hand, and the menu is closed.
 */

const TITLE = "The API returns 418 on Mondays";
const FILE = "20260302-091200-the-api-returns-418-on-mondays.md";
const SECTION = '[data-testid="panel-memory"]';
const MOVED = '[data-cause="memory-moved"]';

describe("moving a memory from its row's menu", function () {
  this.timeout(180_000);

  let project = "";
  const journal = () => join(project, "workspaces", "alpha", "memory");
  const shared = () => join(project, "personas", "_shared", "memory");
  /** The memory's file in the shared store, once it is there. */
  const inShared = () =>
    readdirSync(shared()).find((name) => name.endsWith("the-api-returns-418-on-mondays.md"));
  /** Both stores' indexes as they were, for a hand-made put-back. */
  let indexes: [string, string][] = [];

  before(async () => {
    project = await projectRoot();
    indexes = [journal(), shared()].map((dir) => {
      const index = join(dir, "MEMORY.md");
      return [index, readFileSync(index, "utf8")];
    });
    expect(existsSync(join(journal(), FILE))).toBe(true);
    // Memory is a view of the Panel's right half (#1738); a spec before may have left another.
    await showView("Memory");
    await focusWorkspace("alpha");
    await untilSays(SECTION, TITLE);
  });

  after(async () => {
    await closeMenus();
    if ((await tabNames()).includes(TITLE)) await closeTab(TITLE);
    if (!existsSync(join(journal(), FILE))) {
      const moved = inShared();
      if (moved !== undefined) renameSync(join(shared(), moved), join(journal(), FILE));
    }
    // Always: a move and its Undo each rewrite an index, which need not land as it was.
    for (const [index, text] of indexes) writeFileSync(index, text);
  });

  it("offers Move to with every store of the project but the memory's own", async () => {
    await openMenuOn(SECTION, TITLE);
    expect((await menuRows())[0]).toContain("Move to");

    await pressMenuRow("Move to");

    await browser.waitUntil(async () => (await menuRows()).length === 2, {
      timeout: 10_000,
      timeoutMsg: "Move to never opened its submenu",
    });
    const stores = (await menuRows())[1];
    // The shared store and the personas, by the words the tab's Move uses (`storeLabel`).
    expect(stores.some((row) => row.startsWith("shared"))).toBe(true);
    expect(stores.some((row) => row.startsWith("devops — persona"))).toBe(true);
    // Its own store is not a place to move it to.
    expect(stores.some((row) => row.startsWith("alpha — workspace"))).toBe(false);
    await closeMenus();
  });

  it("moves it whole at the pick, asking nothing, and Undo puts back its exact name", async () => {
    await openMenuOn(SECTION, TITLE);
    await pressMenuRow("Move to");
    await browser.waitUntil(async () => (await menuRows()).length === 2, {
      timeout: 10_000,
      timeoutMsg: "Move to never opened its submenu",
    });

    // The submenu is the innermost menu, drawn last.
    await pressMenuRow("shared");

    await $(MOVED).waitForExist({ timeout: 20_000 });
    // Read as text, drawn or not: the line may be in the Inbox while another view is shown.
    expect(await textOf(MOVED)).toContain(`Moved “${TITLE}” to shared memory`);
    // No question first: nothing is lost, and Undo is on the line.
    expect(await $('[role="alertdialog"]').isExisting()).toBe(false);
    // The file moved, whole: gone from the journal, in the shared store with its title.
    await browser.waitUntil(async () => inShared() !== undefined, {
      timeout: 5_000,
      timeoutMsg: "the memory never reached the shared store",
    });
    expect(existsSync(join(journal(), FILE))).toBe(false);
    expect(readFileSync(join(shared(), inShared() ?? ""), "utf8")).toContain(`# ${TITLE}`);

    // Undo is offered for a few seconds (`UNDO_MS`), so it is pressed before anything slower.
    await press(MOVED, "Undo");

    await browser.waitUntil(async () => existsSync(join(journal(), FILE)), {
      timeout: 20_000,
      timeoutMsg: `Undo did not put the memory back as ${FILE}`,
    });
    expect(inShared()).toBeUndefined();
    await $(MOVED).waitForExist({ reverse: true, timeout: 20_000 });
    await untilSays(SECTION, TITLE);
  });
});
