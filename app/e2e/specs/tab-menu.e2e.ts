// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { closeMenus, openMenuOn } from "../acting.js";
import { menuNames, pressMenuPath, ROW, SUB } from "../menus.js";
import { SELECTED_TAB, endChatsStartedHere, startAChat, type Started } from "../started.js";
import { theWindowHoldsTheRail, WINDOW_1280, windowAt } from "../rail.js";

/**
 * **A chat tab's menu, measured in the real WebView.**
 *
 * A row of this menu is one line of short words (`docs/ui-primitives.md`, "Menus"): its note is
 * its tooltip, and only a row that cannot be undone, or one that cannot run, has a line under
 * its words, which wraps and is never cut. What is held here is the engine's and not jsdom's:
 * that a row is one row tall unless it has that line, and then as tall as the two say; that the menu is a handful of rows
 * which ends inside the window without scrolling; that its submenus end inside the window too;
 * and that each word is drawn at its role's size. `ui/menu/Menu.test.tsx` and
 * `Menus.test.tsx` hold the rules these come from, the second with twelve personas, which this
 * run's project does not have.
 *
 * **Ranges, never exact pixels.** The menu is opened with the event a right-click would have
 * sent (`acting.ts`), and a row is found by its name (`menus.ts`).
 */

/** What the innermost open menu measures: its box, its rows' boxes and sizes, and the window's. */
async function measured() {
  return browser.execute(() => {
    const menus = document.querySelectorAll<HTMLElement>('[role="menu"]');
    const menu = menus[menus.length - 1];
    if (!menu) return null;
    // The surface the list stands on, which the window caps; the list scrolls inside it.
    const surface = menu.closest<HTMLElement>('[data-scope="popover"][data-part="root"]') ?? menu;
    const box = surface.getBoundingClientRect();
    // A control's height, as the page's own tokens give it: what one row is at least.
    const probe = document.createElement("div");
    probe.style.cssText = "position:absolute;visibility:hidden;block-size:var(--size-control)";
    document.body.appendChild(probe);
    const control = probe.getBoundingClientRect().height;
    probe.remove();
    return {
      top: box.top,
      bottom: box.bottom,
      left: box.left,
      right: box.right,
      width: box.width,
      window: window.innerHeight,
      across: window.innerWidth,
      cap: getComputedStyle(surface).maxHeight,
      scrolls: getComputedStyle(menu).overflowY,
      inside: menu.scrollHeight,
      shown: menu.clientHeight,
      body: parseFloat(getComputedStyle(document.body).fontSize),
      root: parseFloat(getComputedStyle(document.documentElement).fontSize),
      control,
      rows: [...menu.querySelectorAll<HTMLElement>('[role^="menuitem"]')].map((row) => {
        const at = row.getBoundingClientRect();
        const title = row.querySelector<HTMLElement>('[data-part="label"]');
        const note = row.querySelector<HTMLElement>('[data-part="description"]');
        const keys = row.querySelector<HTMLElement>('[data-part="shortcut"]');
        return {
          name: row.getAttribute("aria-label") ?? "",
          top: at.top,
          bottom: at.bottom,
          height: at.height,
          // What the row's content needs against what the row was given.
          needs: row.scrollHeight,
          given: row.clientHeight,
          title: title ? parseFloat(getComputedStyle(title).fontSize) : NaN,
          note: note ? parseFloat(getComputedStyle(note).fontSize) : null,
          noteBottom: note ? note.getBoundingClientRect().bottom : null,
          // Whether what is under the row's words is cut: clipped sideways, or by an ellipsis.
          noteCut: note
            ? note.scrollWidth > note.clientWidth + 1 ||
              getComputedStyle(note).textOverflow === "ellipsis" ||
              getComputedStyle(note).whiteSpace === "nowrap"
            : false,
          noteRight: note ? note.getBoundingClientRect().right : null,
          right: at.right,
          keys: keys ? parseFloat(getComputedStyle(keys).fontSize) : null,
          danger: row.getAttribute("data-variant") === "danger",
          off: row.getAttribute("aria-disabled") === "true",
          opens: row.getAttribute("aria-haspopup") === "menu",
          tooltip: row.getAttribute("title"),
        };
      }),
    };
  });
}

type Measured = NonNullable<Awaited<ReturnType<typeof measured>>>;

/** The rows that are squeezed or drawn over the next one, each said with its name: none. */
function rowsAmiss(drawn: Measured): string[] {
  const amiss: string[] = [];
  drawn.rows.forEach((row, i) => {
    // A row holds all of what it says: the line under its words ends inside it.
    if (row.given < row.needs - 1) amiss.push(`${row.name} is ${row.given} tall for ${row.needs}`);
    if (row.noteBottom !== null && row.noteBottom > row.bottom + 1)
      amiss.push(`${row.name}'s note ends under it`);
    const next = drawn.rows[i + 1];
    if (next && row.bottom > next.top + 1) amiss.push(`${row.name} is drawn over ${next.name}`);
  });
  return amiss;
}

/** The rows taller than one row that have no line under their words to be taller for: none. */
function tallerThanARow(drawn: Measured): string[] {
  return drawn.rows
    .filter((row) => row.note === null && row.height > drawn.control * 1.25)
    .map((row) => `${row.name} is ${row.height} tall, and a row is ${drawn.control}`);
}

/** The rows with a line under their words that neither cannot be undone nor cannot run: none. */
function notedForNothing(drawn: Measured): string[] {
  return drawn.rows
    .filter((row) => row.note !== null && !row.danger && !row.off)
    .map((row) => row.name);
}

describe("a chat tab's menu", () => {
  // At 1280×800, where this was held to fit before the default window grew (`rail.ts`,
  // #3077); the project rail hides itself there beside both sidebars.
  before(() => windowAt(WINDOW_1280));
  after(() => theWindowHoldsTheRail());

  let mine: Started | undefined;

  before(async () => {
    mine = await startAChat();
  });

  afterEach(async () => {
    await closeMenus();
  });

  after(async () => {
    await endChatsStartedHere([mine]);
  });

  it("draws one row per line, and a line under a row only where it cannot be undone or cannot run", async () => {
    await openMenuOn(SELECTED_TAB);
    const drawn = await measured();
    if (drawn === null) throw new Error("no menu is open");

    expect(drawn.rows.length).toBeGreaterThan(2);
    expect(rowsAmiss(drawn)).toEqual([]);
    expect(tallerThanARow(drawn)).toEqual([]);
    expect(notedForNothing(drawn)).toEqual([]);
    // What is under a row is never cut: a warning keeps its "There is no undo.", wrapped
    // inside the row where the menu is too narrow for it on one line.
    expect(drawn.rows.filter((row) => row.noteCut).map((row) => row.name)).toEqual([]);
    expect(
      drawn.rows
        .filter((row) => row.noteRight !== null && row.noteRight > row.right + 1)
        .map((row) => row.name),
    ).toEqual([]);
    // The row that ends the chat is there, and says what it costs.
    expect(drawn.rows.filter((row) => row.danger && row.note !== null).length).toBeGreaterThan(0);
  });

  it("is a handful of rows that say no tab's name, with the kinds of row folded into submenus", async () => {
    const name = await browser.execute(
      (selected: string) =>
        document.querySelector(`${selected} .tab-name`)?.textContent?.trim() ?? "",
      SELECTED_TAB,
    );
    await openMenuOn(SELECTED_TAB);
    const drawn = await measured();
    if (drawn === null) throw new Error("no menu is open");

    // However many personas the project has, they are behind one row.
    expect(drawn.rows.length).toBeLessThanOrEqual(10);
    expect(name).not.toBe("");
    expect(drawn.rows.filter((row) => row.name.includes(name)).map((row) => row.name)).toEqual([]);
    const names = drawn.rows.map((row) => row.name);
    expect(names).toContain(ROW.rename);
    expect(names).toContain(ROW.pinned);
    // The tab in front has no row that could only say it is in front.
    expect(names).not.toContain(ROW.switchTo);
    // Activity and Network are views of the chat: one row, which opens them.
    expect(names).not.toContain(ROW.activity);
    expect(drawn.rows.find((row) => row.name === SUB.open)?.opens).toBe(true);
    // No platform tooltip repeats a row's words or its note: a row that can run has none.
    expect(
      drawn.rows.filter((row) => !row.off && row.tooltip !== null).map((row) => row.name),
    ).toEqual([]);
  });

  it("writes each word at its role's size: a row at the body's, a line under it and a key smaller", async () => {
    await openMenuOn(SELECTED_TAB);
    const drawn = await measured();
    if (drawn === null) throw new Error("no menu is open");

    // The body role is no larger than the window's own text.
    expect(drawn.rows.filter((row) => !(row.title <= drawn.body)).map((row) => row.name)).toEqual(
      [],
    );
    const noted = drawn.rows.filter((row) => row.note !== null);
    expect(noted.filter((row) => !((row.note ?? NaN) < row.title)).map((row) => row.name)).toEqual(
      [],
    );
    const keyed = drawn.rows.filter((row) => row.keys !== null);
    expect(keyed.length).toBeGreaterThan(0);
    expect(keyed.filter((row) => !((row.keys ?? NaN) < row.title)).map((row) => row.name)).toEqual(
      [],
    );
  });

  it("ends inside the window without scrolling, no narrower than its least width nor wider than its most", async () => {
    await openMenuOn(SELECTED_TAB);
    const drawn = await measured();
    if (drawn === null) throw new Error("no menu is open");

    // Its cap is the room the window has for it: a length, and no more than the window.
    expect(drawn.top).toBeGreaterThanOrEqual(0);
    expect(drawn.bottom).toBeLessThanOrEqual(drawn.window);
    expect(drawn.left).toBeGreaterThanOrEqual(0);
    expect(drawn.right).toBeLessThanOrEqual(drawn.across);
    expect(drawn.cap).toMatch(/px$/);
    expect(parseFloat(drawn.cap)).toBeLessThanOrEqual(drawn.window);
    expect(drawn.scrolls).toBe("auto");
    // A handful of one-line rows fits the window: nothing is past the menu's own edge.
    expect(drawn.inside).toBeLessThanOrEqual(drawn.shown + 1);
    // Between the one menu style's least and most width: six and ten of the largest space
    // step, which is 32px at the root's default 14px, with room for rounding.
    const step = (drawn.root / 14) * 32;
    expect(drawn.width).toBeGreaterThanOrEqual(step * 6 - 2);
    expect(drawn.width).toBeLessThanOrEqual(step * 10 + 2);
  });

  it("opens Open beside it, inside the window, with a row per view", async () => {
    await openMenuOn(SELECTED_TAB);
    await pressMenuPath(SUB.open);
    await browser.waitUntil(async () => (await menuNames()).length === 2, {
      timeout: 10_000,
      timeoutMsg: "Open opened no submenu",
    });
    const sub = await measured();
    if (sub === null) throw new Error("the submenu closed");

    expect(sub.rows.map((row) => row.name)).toEqual([ROW.activity, ROW.network]);
    expect(sub.top).toBeGreaterThanOrEqual(0);
    expect(sub.bottom).toBeLessThanOrEqual(sub.window);
    expect(sub.right).toBeLessThanOrEqual(sub.across);
    expect(rowsAmiss(sub)).toEqual([]);
    expect(tallerThanARow(sub)).toEqual([]);
    expect(notedForNothing(sub)).toEqual([]);
  });

  it("scrolls where it is given less room than its rows need, and squeezes none of them", async () => {
    await openMenuOn(SELECTED_TAB);
    const drawn = await measured();
    if (drawn === null) throw new Error("no menu is open");

    // Given less room than its rows need, as a short window gives it, it scrolls: the rows
    // keep their size and stand apart as before.
    await browser.execute(
      (room: number) => {
        const menu = document.querySelector<HTMLElement>('[role="menu"]');
        if (menu) menu.style.maxHeight = `${room}px`;
      },
      Math.floor(drawn.inside / 2),
    );
    const short = await measured();
    if (short === null) throw new Error("the menu closed when it was given less room");
    expect(short.inside).toBeGreaterThan(short.shown);
    expect(rowsAmiss(short)).toEqual([]);
    expect(
      drawn.rows.filter((row, i) => short.rows[i].given < row.given - 1).map((row) => row.name),
    ).toEqual([]);
  });
});
