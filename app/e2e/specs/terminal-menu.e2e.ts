// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { closeMenus, openMenuOn, pressMenuRow } from "../acting.js";
import { endChatsStartedHere, startAChat, type Started } from "../started.js";
import { theWindowHoldsTheRail, WINDOW_1280, windowAt } from "../rail.js";

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
        const title = row
          .querySelector<HTMLElement>('[data-part="label"]')
          ?.getBoundingClientRect();
        const key = row
          .querySelector<HTMLElement>('[data-part="shortcut"]')
          ?.getBoundingClientRect();
        return {
          name: ((row: Element) => {
            // A row's name as a reader hears it (#6043): its words, without what is kept from
            // a reader (the line under them, its key).
            const own = row.getAttribute("aria-label");
            if (own !== null) return own;
            const heard = row.cloneNode(true) as Element;
            for (const kept of heard.querySelectorAll('[aria-hidden="true"], [hidden]'))
              kept.remove();
            return (heard.textContent ?? "").trim();
          })(row),
          off: row.getAttribute("aria-disabled") === "true",
          // Why it cannot run: its description, which its words also say under them.
          why:
            (row.getAttribute("aria-describedby") ?? "")
              .split(/\s+/)
              .map((id) => document.getElementById(id)?.textContent ?? "")
              .join(" ")
              .trim() || null,
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
  // At 1280×800, where this was held to fit before the default window grew (`rail.ts`,
  // #3077); the project rail hides itself there beside both sidebars.
  before(() => windowAt(WINDOW_1280));
  after(() => theWindowHoldsTheRail());

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

    // Away from a link, *Links in this pane…* leads (#6219), then the terminal's own rows.
    expect(menu.rows.slice(0, 4).map((row) => row.name)).toEqual([
      "Links in this pane…",
      "Copy",
      "Paste",
      "Select all",
    ]);
    // The pane's own rows are still here: a right-click on a pane strands none of them.
    expect(menu.rows.length).toBeGreaterThan(4);
    expect(menu.lines).toBeGreaterThanOrEqual(2);

    // Paste and Select all can always run.
    expect(menu.rows[2].off).toBe(false);
    expect(menu.rows[3].off).toBe(false);
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
    const copy = menu.rows.find((row) => row.name === "Copy");
    expect(copy?.off).toBe(false);

    await pressMenuRow("Copy");
    await browser.waitUntil(async () => (await drawn()) === null, {
      timeout: 10_000,
      timeoutMsg: "the menu stayed open after Copy",
    });
  });
});
