// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, readFileSync, realpathSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { ask, inFront, openFromTheStrip, PROJECTS } from "../switching.js";

/**
 * **Settings' tables keep each heading on one line, whole, in the real WebView** (DS-9, #1841;
 * spec #1832's seam 3), and stack into a form only where Settings is narrow (#1859, #1865). Both
 * Dispatch tables wear the shared `.ui-table`: a heading never wraps,
 * and a long one is shortened (its whole words in its tooltip), so at the default window width no
 * heading is cut either.
 *
 * What jsdom cannot say: where WebKit breaks a line and how wide a word is drawn.
 * `DispatchLimits.test.tsx` and `DispatchGrants.test.tsx` hold what the cascade gives each
 * heading. Ranges, never pixels: a heading's words make one line box, and its words fit its cell.
 *
 * **Its own project**, copied into the run's tree: the fixture has one persona, and the grants
 * table is drawn only once the project grants something, so this copy adds a second persona and
 * a project grant from it. Nothing is written: the spec only measures, then closes the project's
 * Settings tab and the project, and brings the launch's project back to the front.
 */

const TABS = '[data-strip="Tabs"]';
const LEVEL = '[role="radiogroup"][aria-label="Level"]';
const GROUPS = 'nav[aria-label="Groups"]';
const LIMITS = 'table[aria-label="Dispatch limits"]';
const GRANTS = "table.dispatch-table";

/** This spec's project: the fixture with a second persona, `steward`, granted `devops`. */
const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), "settings-tables");
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  const devops = readFileSync(join(root, "personas", "devops", "persona.md"), "utf8");
  mkdirSync(join(root, "personas", "steward"), { recursive: true });
  writeFileSync(
    join(root, "personas", "steward", "persona.md"),
    devops.replace(/^name: devops$/m, "name: steward").replace(/devops/g, "steward"),
  );
  const manifest = join(root, "charter.toml");
  writeFileSync(
    manifest,
    `${readFileSync(manifest, "utf8").trimEnd()}\n\n[dispatch.grants]\nsteward = ["devops"]\n`,
  );
  anEmptyRecord(root);
  return root;
})();

/** One heading as drawn: its words, how many line boxes they make, and whether they are cut. */
type Heading = { words: string; lines: number; cut: boolean };

/** Every column heading of the table at `table`, measured; none where it is not drawn. */
async function headingsOf(table: string): Promise<Heading[] | null> {
  return browser.execute((selector: string) => {
    const drawn = document.querySelector(selector);
    if (drawn === null) return null;
    return [...drawn.querySelectorAll<HTMLElement>("thead th")].map((th) => {
      const range = document.createRange();
      range.selectNodeContents(th);
      // A line box per distinct top among the words' boxes, a pixel or two of rounding apart.
      const tops = [...range.getClientRects()]
        .filter((one) => one.width > 0)
        .map((one) => one.top)
        .sort((a, b) => a - b);
      let lines = 0;
      let last = Number.NEGATIVE_INFINITY;
      for (const top of tops)
        if (top - last > 2) {
          lines += 1;
          last = top;
        }
      return {
        words: th.textContent?.trim() ?? "",
        lines,
        cut: th.scrollWidth > th.clientWidth + 1,
      };
    });
  }, table);
}

/** The two sidebars: each one's activity bar, and the slot it opens. */
const SIDES = [
  ["Navigation", "region-left"],
  ["Attention", "region-right"],
] as const;

/** Sets the window's size and waits until the page has settled at its width. */
async function windowIs(width: number, height: number): Promise<void> {
  await browser.setWindowSize(width, height);
  let last = -1;
  await browser.waitUntil(
    async () => {
      const now = await browser.execute(() => window.innerWidth);
      const settled = now === last;
      last = now;
      return settled;
    },
    { timeout: 10_000, interval: 200, timeoutMsg: "the window never settled on a width" },
  );
}

/**
 * Opens each sidebar that is put away, by a press of a view's icon on its activity bar, so the
 * narrowest case does not depend on what a spec run before it left (as `settings-limits.e2e.ts`
 * does). Returns each icon it pressed, for {@link putAwayAgain}.
 */
async function bothSidesOpen(): Promise<string[]> {
  const pressed: string[] = [];
  for (const [bar, slot] of SIDES) {
    const side = await $(`[data-panel][id="${slot}"]`);
    if (((await side.getSize("width")) as number) > 0) continue;
    const icons = `[role="tablist"][aria-label="${bar}"] [role="tab"]`;
    const open = await $(`${icons}[aria-selected="true"]`);
    const icon = (await open.isExisting()) ? open : await $(icons);
    const name = (await icon.getAttribute("aria-label")) ?? "";
    await icon.click();
    await browser.waitUntil(async () => ((await side.getSize("width")) as number) > 0, {
      timeout: 20_000,
      timeoutMsg: `the ${bar} side did not open when its view was pressed`,
    });
    pressed.push(`${bar}\u0000${name}`);
  }
  return pressed;
}

/** Puts away again each sidebar {@link bothSidesOpen} opened: a press of its open view. */
async function putAwayAgain(pressed: string[]): Promise<void> {
  for (const one of pressed) {
    const [bar, name] = one.split("\u0000");
    await (
      await $(`[role="tablist"][aria-label="${bar}"] [role="tab"][aria-label="${name}"]`)
    ).click();
  }
}

/**
 * The room the table at `table` answers to (#1859): its width, whether the table's headings are
 * put away (it stacks) and whether it scrolls sideways; and each body cell that carries its column's words, measured,
 * with what is drawn above it. `null` where it is not drawn.
 */
async function roomOf(table: string) {
  return browser.execute((selector: string) => {
    const drawn = document.querySelector(selector);
    const room = drawn?.closest(".ui-table-room");
    const head = drawn?.querySelector("thead");
    if (!drawn || !room || !head) return null;
    const box = (el: Element) => {
      const { left, right, top, bottom } = el.getBoundingClientRect();
      return { left, right, top, bottom };
    };
    return {
      width: room.clientWidth,
      room: box(room),
      headings: getComputedStyle(head).display,
      scrolls: room.scrollWidth > room.clientWidth + 1,
      cells: [...drawn.querySelectorAll<HTMLElement>("tbody td[data-heading]")]
        .filter((cell) => cell.childElementCount > 0 || (cell.textContent ?? "").trim() !== "")
        .map((cell) => ({
          box: box(cell),
          said: getComputedStyle(cell, "::before").content,
          carried: JSON.stringify(cell.dataset.heading ?? ""),
          cut: cell.scrollWidth > cell.clientWidth + 1,
        })),
    };
  }, table);
}

/** Opens the group called `label` from the nav, and waits for it to be the one shown. */
async function openGroup(label: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (nav: string, wanted: string) => {
          const button = [...document.querySelectorAll<HTMLElement>(`${nav} button`)].find(
            (one) => one.textContent?.trim() === wanted,
          );
          if (button?.getAttribute("aria-current") === "true") return true;
          button?.click();
          return false;
        },
        GROUPS,
        label,
      ),
    { timeout: 20_000, interval: 250, timeoutMsg: `Settings offered no group ${label}` },
  );
}

/** Waits for the table at `table` and answers its headings, measured. */
async function measured(table: string): Promise<Heading[]> {
  let seen: Heading[] | null = null;
  await browser.waitUntil(
    async () => {
      seen = await headingsOf(table);
      return seen !== null && seen.length > 0;
    },
    { timeout: 30_000, interval: 250, timeoutMsg: `Settings never drew ${table}` },
  );
  return seen as unknown as Heading[];
}

/** Every heading drawn on one line and whole: each named, so a failure says which. */
function oneLineEach(headings: readonly Heading[]): void {
  for (const one of headings) {
    // The action column's heading has no words, so no line box.
    expect({ heading: one.words, lines: Math.min(one.lines, 2) }).toEqual({
      heading: one.words,
      lines: one.words === "" ? 0 : 1,
    });
    expect({ heading: one.words, cut: one.cut }).toEqual({ heading: one.words, cut: false });
  }
}

describe("Settings' tables, in the real WebView", function () {
  this.timeout(180_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";
  let was = { width: 1280, height: 800 };

  before(async () => {
    was = await browser.getWindowSize();
    // The default window's size (`settings-limits.e2e.ts` measures the page at it too).
    await browser.setWindowSize(1280, 800);
    first = (await ask<string[]>("open_planes"))[0];
    await openFromTheStrip(mine);

    await browser.executeAsync((done: () => void) => {
      void window.__TAURI__.event.emit("settings-asked").then(done, done);
    });
    await $(LEVEL).waitForExist({ timeout: 30_000 });
    await browser.waitUntil(
      async () =>
        browser.execute((selector: string) => {
          const radios = [...document.querySelectorAll<HTMLElement>(`${selector} [role="radio"]`)];
          const project = radios.find((one) => one.textContent?.trim() === "Project");
          if (project?.getAttribute("aria-checked") === "true") return true;
          project?.click();
          return false;
        }, LEVEL),
      { timeout: 30_000, interval: 250, timeoutMsg: "Settings never came to the Project level" },
    );
    await openGroup("Dispatch");
  });

  after(async () => {
    const tried = async (step: () => Promise<unknown>) => {
      try {
        await step();
      } catch {
        // The next step still runs.
      }
    };
    await tried(() =>
      browser.execute((strip: string) => {
        for (const closer of document.querySelectorAll<HTMLElement>(
          `${strip} button[aria-label^="Close Settings"]`,
        ))
          closer.click();
      }, TABS),
    );
    await tried(async () => {
      for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
        await ask("close_plane", { plane });
    });
    await tried(() =>
      browser.waitUntil(
        async () => {
          if ((await inFront()) === first) return true;
          await browser.execute(
            (strip: string, path: string) =>
              [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)]
                .find((tab) => tab.getAttribute("data-directory") === path)
                ?.click(),
            PROJECTS,
            first,
          );
          return false;
        },
        { timeout: 20_000, interval: 250, timeoutMsg: "the launch's project never came back" },
      ),
    );
    await tried(() => browser.setWindowSize(was.width, was.height));
  });

  it("draws every heading of the limits table on one line, whole", async () => {
    const headings = await measured(LIMITS);
    expect(headings.map((one) => one.words)).toContain("Messages");
    oneLineEach(headings);
  });

  it("draws every heading of the grants table on one line, whole", async () => {
    const headings = await measured(GRANTS);
    expect(headings.map((one) => one.words)).toEqual([
      "Dispatch to",
      "Where it comes from",
      "Workspace",
      "Action",
    ]);
    oneLineEach(headings);
  });

  /**
   * **The width they stack at, against the real window** (#1865): 29rem was set between
   * Settings' body at the default window and at the narrowest (#1859), both estimated from the
   * layout. CI's runs of this spec (a table at the default window, its headings each on one line)
   * and of `settings-limits.e2e.ts` (a form at the narrowest) held it; this says it outright, for
   * both tables.
   */
  it("keeps both tables as tables at the default window", async () => {
    for (const table of [LIMITS, GRANTS]) {
      await measured(table);
      const seen = await roomOf(table);
      if (seen === null) throw new Error(`${table} is not drawn in a room`);
      expect({ table, headings: seen.headings }).not.toEqual({ table, headings: "none" });
    }
  });

  it("stacks the grants table into a form at the narrowest Settings: each cell under its column's words, none cut", async () => {
    // The narrowest Settings by default: the narrowest window (1024 px, ADR 0054) with both
    // sidebars open, whatever the specs before this one left, and put back after.
    const opened = await bothSidesOpen();
    try {
      await windowIs(1024, 768);
      await measured(GRANTS);
      let seen = await roomOf(GRANTS);
      await browser.waitUntil(
        async () => {
          seen = await roomOf(GRANTS);
          return seen !== null && seen.headings === "none";
        },
        { timeout: 10_000, timeoutMsg: "the grants table did not stack at the narrowest Settings" },
      );
      if (seen === null) throw new Error("the grants table is not drawn in a room");
      const { room, cells } = seen;
      expect(seen.scrolls).toBe(false);
      expect(cells.length).toBeGreaterThanOrEqual(1);
      for (const cell of cells) {
        // Inside the room, give or take a pixel of rounding, under its column's whole words.
        expect(cell.box.left).toBeGreaterThanOrEqual(room.left - 1);
        expect(cell.box.right).toBeLessThanOrEqual(room.right + 1);
        expect(cell.said).toBe(cell.carried);
        expect(cell.cut).toBe(false);
      }
    } finally {
      await putAwayAgain(opened);
      await windowIs(1280, 800);
    }
  });
});
