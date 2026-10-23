// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { closeMenus, openMenuOn, pressMenuRow } from "../acting.js";
import { endChatsStartedHere, startAChat, type Started } from "../started.js";

/**
 * **A terminal's menu, in the real WebView**: a right-click on a chat's terminal offers Copy,
 * Paste and Select all first, and the pane's own rows under them.
 *
 * `SessionPane.menu.test.tsx` holds what each row does, against xterm and a clipboard that
 * records. What is held here is the engine's: that the event a right-click sends reaches the
 * terminal's menu through xterm's own element and not the panes', that the rows are drawn
 * inside the window with a key at the row's end and not over its words, and that Select all
 * is what turns Copy on.
 *
 * **The clipboard itself is not read here, and Paste is not pressed.** The page cannot read
 * the clipboard in either engine without the engine's say, and Paste would type whatever the
 * machine running this happens to hold into a chat; that Copy wrote exactly the selection, and
 * that Paste goes once through the terminal's own paste, are the jsdom test's. The menu is opened with the event a right-click
 * would have sent (`acting.ts`). Ranges, never exact pixels.
 */

/** The open menu's rows: each one's name, whether it can run, and where its parts are. */
async function drawn() {
  return browser.execute(() => {
    const menu = document.querySelector<HTMLElement>('[role="menu"]');
    if (!menu) return null;
    const box = menu.getBoundingClientRect();
    return {
      left: box.left,
      right: box.right,
      top: box.top,
      bottom: box.bottom,
      wide: window.innerWidth,
      tall: window.innerHeight,
      lines: menu.querySelectorAll('[role="separator"]').length,
      rows: [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')].map((row) => {
        const at = row.getBoundingClientRect();
        const title = row.querySelector<HTMLElement>(".ui-menu-title")?.getBoundingClientRect();
        const key = row.querySelector<HTMLElement>(".ui-menu-keys")?.getBoundingClientRect();
        return {
          name: row.getAttribute("aria-label") ?? "",
          off: row.getAttribute("aria-disabled") === "true",
          // Why it cannot run: its description, which its words also say under them.
          why:
            document.getElementById(row.getAttribute("aria-describedby") ?? "")?.textContent ??
            null,
          right: at.right,
          titleRight: title?.right ?? null,
          keyLeft: key?.left ?? null,
          keyRight: key?.right ?? null,
        };
      }),
    };
  });
}

describe("a terminal's menu", () => {
  let mine: Started | undefined;
  const terminal = () => `[data-testid="pane"][data-session="${mine?.session}"] .xterm-screen`;

  before(async () => {
    mine = await startAChat();
    await $(terminal()).waitForExist({ timeout: 20_000 });
  });

  afterEach(async () => {
    await closeMenus();
  });

  after(async () => {
    await endChatsStartedHere([mine]);
  });

  it("offers the terminal's rows first, then the pane's under a line", async () => {
    await openMenuOn(terminal());
    const menu = await drawn();
    if (menu === null) throw new Error("no menu is open");

    expect(menu.rows.slice(0, 3).map((row) => row.name)).toEqual(["Copy", "Paste", "Select all"]);
    // The pane's own rows are still here: a right-click on a pane strands none of them.
    expect(menu.rows.length).toBeGreaterThan(3);
    expect(menu.lines).toBeGreaterThanOrEqual(1);

    // Paste and Select all can always run.
    expect(menu.rows[1].off).toBe(false);
    expect(menu.rows[2].off).toBe(false);
  });

  it("is drawn inside the window, with a key at a row's end and not over its words", async () => {
    await openMenuOn(terminal());
    const menu = await drawn();
    if (menu === null) throw new Error("no menu is open");

    expect(menu.left).toBeGreaterThanOrEqual(0);
    expect(menu.top).toBeGreaterThanOrEqual(0);
    expect(menu.right).toBeLessThanOrEqual(menu.wide);
    expect(menu.bottom).toBeLessThanOrEqual(menu.tall);

    // A key is drawn only where the window has one (a Mac's Edit menu), so there may be none.
    const keyed = menu.rows.filter((row) => row.keyLeft !== null);
    expect(
      keyed
        .filter(
          (row) =>
            !((row.keyLeft ?? 0) >= (row.titleRight ?? 0) && (row.keyRight ?? 0) <= row.right + 1),
        )
        .map((row) => row.name),
    ).toEqual([]);
  });

  it("turns Copy on once Select all has selected, and Copy closes the menu", async () => {
    await openMenuOn(terminal());
    await pressMenuRow("Select all");
    await browser.waitUntil(async () => (await drawn()) === null, {
      timeout: 10_000,
      timeoutMsg: "the menu stayed open after Select all",
    });

    await openMenuOn(terminal());
    const menu = await drawn();
    if (menu === null) throw new Error("no menu is open");
    expect(menu.rows[0].name).toBe("Copy");
    expect(menu.rows[0].off).toBe(false);

    await pressMenuRow("Copy");
    await browser.waitUntil(async () => (await drawn()) === null, {
      timeout: 10_000,
      timeoutMsg: "the menu stayed open after Copy",
    });
  });
});
