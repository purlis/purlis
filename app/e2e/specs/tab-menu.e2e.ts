// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { closeMenus, openMenuOn } from "../acting.js";
import { SELECTED_TAB, endChatsStartedHere, startAChat, type Started } from "../started.js";

/**
 * **A chat tab's menu, measured in the real WebView.**
 *
 * A row of this menu is a title with a note under it, and the note wraps. What is held here is
 * the engine's and not jsdom's: that each row is as tall as what it says, so no row is drawn
 * over the next; that the menu ends inside the window and scrolls where its rows do not fit;
 * and that a row is written no larger than the window's text. `primitives/controls.test.tsx`
 * holds the rules these come from.
 *
 * **Ranges, never exact pixels.** The menu is opened with the event a right-click would have
 * sent (`acting.ts`).
 */

/** What the open menu measures: its box, its rows' boxes and sizes, and the window's. */
async function measured() {
  return browser.execute(() => {
    const menu = document.querySelector<HTMLElement>('[role="menu"]');
    if (!menu) return null;
    const box = menu.getBoundingClientRect();
    return {
      top: box.top,
      bottom: box.bottom,
      window: window.innerHeight,
      cap: getComputedStyle(menu).maxHeight,
      scrolls: getComputedStyle(menu).overflowY,
      inside: menu.scrollHeight,
      shown: menu.clientHeight,
      body: parseFloat(getComputedStyle(document.body).fontSize),
      rows: [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')].map((row) => {
        const at = row.getBoundingClientRect();
        const title = row.querySelector<HTMLElement>(".menu-title");
        const note = row.querySelector<HTMLElement>(".ui-menu-note");
        return {
          name: row.getAttribute("aria-label") ?? "",
          top: at.top,
          bottom: at.bottom,
          // What the row's content needs against what the row was given.
          needs: row.scrollHeight,
          given: row.clientHeight,
          title: title ? parseFloat(getComputedStyle(title).fontSize) : NaN,
          note: note ? parseFloat(getComputedStyle(note).fontSize) : null,
          noteBottom: note ? note.getBoundingClientRect().bottom : null,
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
    // A row holds all of what it says: its note ends inside it.
    if (row.given < row.needs - 1) amiss.push(`${row.name} is ${row.given} tall for ${row.needs}`);
    if (row.noteBottom !== null && row.noteBottom > row.bottom + 1)
      amiss.push(`${row.name}'s note ends under it`);
    const next = drawn.rows[i + 1];
    if (next && row.bottom > next.top + 1) amiss.push(`${row.name} is drawn over ${next.name}`);
  });
  return amiss;
}

describe("a chat tab's menu", () => {
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

  it("draws each row as tall as what it says, and none over the next", async () => {
    await openMenuOn(SELECTED_TAB);
    const drawn = await measured();
    if (drawn === null) throw new Error("no menu is open");

    // Some rows carry a note under their words: those are the rows that were squeezed.
    expect(drawn.rows.length).toBeGreaterThan(2);
    expect(drawn.rows.some((row) => row.note !== null)).toBe(true);
    expect(rowsAmiss(drawn)).toEqual([]);
  });

  it("writes a row no larger than the window's text, and its note smaller, said once", async () => {
    await openMenuOn(SELECTED_TAB);
    const drawn = await measured();
    if (drawn === null) throw new Error("no menu is open");

    const noted = drawn.rows.filter((row) => row.note !== null);
    expect(noted.length).toBeGreaterThan(0);
    expect(drawn.rows.filter((row) => !(row.title <= drawn.body)).map((row) => row.name)).toEqual(
      [],
    );
    expect(noted.filter((row) => !((row.note ?? NaN) < row.title)).map((row) => row.name)).toEqual(
      [],
    );
    // The note is under the row's words, so no tooltip says it again.
    expect(noted.filter((row) => row.tooltip !== null).map((row) => row.name)).toEqual([]);
  });

  it("ends inside the window, and scrolls where its rows do not fit", async () => {
    await openMenuOn(SELECTED_TAB);
    const drawn = await measured();
    if (drawn === null) throw new Error("no menu is open");

    // Its cap is the room the window has for it: a length, and no more than the window.
    expect(drawn.top).toBeGreaterThanOrEqual(0);
    expect(drawn.bottom).toBeLessThanOrEqual(drawn.window);
    expect(drawn.cap).toMatch(/px$/);
    expect(parseFloat(drawn.cap)).toBeLessThanOrEqual(drawn.window);
    expect(drawn.scrolls).toBe("auto");

    // Given less room than its rows need, as a short window or a long list of personas gives
    // it, it scrolls: the rows keep their size and stand apart as before.
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
