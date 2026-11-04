// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { closeMenus, menuRows, openMenuOn, pressMenuRow } from "../acting.js";
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

// DIAGNOSTIC (#5193, dropped before merge): every round's first test is right after a chat starts.
const ROUNDS = 20;
for (let round = 1; round <= ROUNDS; round++)
  describe(`a terminal's menu (round ${round})`, () => {
    // At 1280×800, where this was held to fit before the default window grew (`rail.ts`,
    // #3077); the project rail hides itself there beside both sidebars.
    before(() => windowAt(WINDOW_1280));
    after(() => theWindowHoldsTheRail());

    let mine: Started | undefined;
    const terminal = () => `[data-testid="pane"][data-session="${mine?.session}"] .xterm-screen`;

    before(async () => {
      await browser.execute(() => {
        const w = window as unknown as { __focusLog?: string[]; __focusHeard?: boolean };
        w.__focusLog = [];
        if (w.__focusHeard) return;
        w.__focusHeard = true;
        const t0 = performance.now();
        for (const kind of ["focusin", "focusout"])
          document.addEventListener(
            kind,
            (event) => {
              const el = event.target as Element;
              const said = `${Math.round(performance.now() - t0)}ms ${kind} ${el.tagName?.toLowerCase()}.${String(el.className).slice(0, 40)} role=${el.getAttribute?.("role")} :: ${(new Error().stack ?? "").split("\n").slice(1, 9).join(" <- ")}`;
              w.__focusLog?.push(said);
              if ((w.__focusLog?.length ?? 0) > 60) w.__focusLog?.shift();
            },
            true,
          );
      });
      mine = await startAChat();
      await $(terminal()).waitForExist({ timeout: 20_000 });
    });

    afterEach(async () => {
      try {
        await closeMenus();
      } catch (error) {
        const seen = await browser.execute(() => {
          const name = (el: Element | null) =>
            el === null
              ? "null"
              : `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ""}.${String(el.className).slice(0, 60)} role=${el.getAttribute("role")} aria-label=${el.getAttribute("aria-label")}`;
          const w = window as unknown as { __focusLog?: string[] };
          return {
            active: name(document.activeElement),
            menus: document.querySelectorAll('[role="menu"]').length,
            inMenu: !!document.activeElement?.closest('[role="menu"]'),
            log: (w.__focusLog ?? []).slice(-12),
          };
        });
        throw new Error(`${String(error)}\n#5193 diagnosis: ${JSON.stringify(seen, null, 1)}`);
      }
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
              !(
                (row.keyLeft ?? 0) >= (row.titleRight ?? 0) && (row.keyRight ?? 0) <= row.right + 1
              ),
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

// DIAGNOSTIC (#5193, dropped before merge): the race pinned. A transition is held running while
// the menu opens, as one is just after a chat starts.
describe("DIAG #5193: a terminal's menu opened while the window is moving", () => {
  before(() => windowAt(WINDOW_1280));
  after(() => theWindowHoldsTheRail());
  let mine: Started | undefined;
  const terminal = () => `[data-testid="pane"][data-session="${mine?.session}"] .xterm-screen`;
  const inMenu = () =>
    browser.execute(() => Boolean(document.activeElement?.closest('[role="menu"]')));
  const moving = async () => {
    await browser.execute(() => {
      const held = document.createElement("div");
      held.id = "diag-5193-moving";
      held.style.cssText =
        "position:fixed;left:0;top:0;width:1px;height:1px;opacity:0;transition:opacity 30s linear";
      document.body.appendChild(held);
      void held.offsetWidth;
      held.style.opacity = "1";
    });
    // A press from the page, as `pressOnly` makes one.
    await browser.execute(() => document.body.click());
    await browser.pause(500);
  };
  const still = () => browser.execute(() => document.getElementById("diag-5193-moving")?.remove());

  before(async () => {
    mine = await startAChat();
    await $(terminal()).waitForExist({ timeout: 20_000 });
  });
  afterEach(async () => {
    await still();
    await closeMenus();
  });
  after(async () => {
    await endChatsStartedHere([mine]);
  });

  it("before the fix: a contextmenu alone leaves the keyboard out of the menu, and Escape closes nothing", async () => {
    await moving();
    await browser.execute((css: string) => {
      const element = document.querySelector<HTMLElement>(css);
      const box = element!.getBoundingClientRect();
      element!.dispatchEvent(
        new MouseEvent("contextmenu", {
          bubbles: true,
          cancelable: true,
          clientX: Math.round(box.left + box.width / 2),
          clientY: Math.round(box.top + box.height / 2),
        }),
      );
    }, terminal());
    await $('[role="menu"]').waitForDisplayed({ timeout: 10_000 });
    await browser.pause(1_000);
    expect(await inMenu()).toBe(false);
    await browser.keys(["Escape"]);
    await browser.pause(1_000);
    expect((await menuRows()).length).toBeGreaterThan(0);
  });

  it("after the fix: openMenuOn's press puts the keyboard in the menu at once, and Escape closes it", async () => {
    await moving();
    await openMenuOn(terminal());
    expect(await inMenu()).toBe(true);
    await browser.keys(["Escape"]);
    await browser.waitUntil(async () => (await menuRows()).length === 0, {
      timeout: 3_000,
      timeoutMsg: "the menu stayed open after one Escape",
    });
  });
});
