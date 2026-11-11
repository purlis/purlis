// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { closesTheTabsItOpens } from "../reach.js";
import { SETTINGS_GROUPS, openProjectSettings } from "../settingsTab.js";
import { theWindowHoldsTheRail, WINDOW_1280, windowAt } from "../rail.js";

/**
 * **A Settings page is the aligned form, in the real WebView** (#2919, prototype A of
 * `prototype/settings-page`, the audit's R37–R62): a walk through Project › Saving as the
 * operator makes it.
 *
 * - The side column, with the level switcher and the pages and no title of its own (the tab
 *   names the view, #3147), beside the page, which scrolls on its own (R37, R38).
 * - The page's header names its file as code, with Edit as TOML after it (R48, R49).
 * - Its groups have headings, and the rarely used ones, Branches and Deprecated, start folded
 *   as "‹title› · ‹count›" with the chevron after the words (R60, R61).
 * - Every control starts at one x and every row's ⋯ stands at one x; each row's help is a line
 *   under its label, in the label's column (R50, R52, R54, R56).
 * - `[memory] share`, which the fixture project's `charter.toml` sets, carries the set-here mark
 *   and the amber deprecated badge (R53, R62); its ⋯ offers Reset in reach and Copy key, and
 *   Escape closes it on its button (R57, R59). A row not set here offers Reset out of reach
 *   (R58).
 * - Edit as TOML in the header opens the file's text (SE-19).
 * - A group in the nav folds from its toggle at the row's end, apart from its page: its pages go
 *   and it is no page chosen; it opens again the same way (R42).
 *
 * What jsdom cannot say: where WebKit lays these out. Ranges, never exact pixels.
 * `SettingsPage.test.tsx` holds the behaviour. **Nothing is written** but the fold of a group,
 * which the layout file keeps and the spec puts back as it was; it closes the Settings tab it
 * opened.
 */

const SET = "[memory] share";
const UNSET = "Target branch";

type Box = { top: number; bottom: number; left: number; right: number; width: number };

/** The page as it is laid out: the side column, the header and every row's columns. */
async function laidOut() {
  return browser.execute(() => {
    const box = (el: Element | null | undefined): Box | null => {
      if (!el) return null;
      const { top, bottom, left, right, width } = el.getBoundingClientRect();
      return { top, bottom, left, right, width };
    };
    const side = document.querySelector(".ui-settings-side");
    const page = document.querySelector(".ui-settings-page section");
    // The rows on screen: a folded group's are drawn and hidden (R61).
    const rows = [...(page?.querySelectorAll<HTMLElement>(".ui-setting-row") ?? [])].filter(
      (row) => row.getClientRects().length > 0,
    );
    return {
      side: box(side),
      whole: box(document.querySelector(".ui-settings")),
      switcher: side?.querySelector('[role="radiogroup"][aria-label="Level"]') != null,
      heading: page?.querySelector("h2")?.textContent ?? "",
      file: page?.querySelector(".ui-setting-where code")?.textContent ?? "",
      edit:
        [...(page?.querySelectorAll(".ui-setting-where button") ?? [])].map(
          (one) => one.textContent ?? "",
        )[0] ?? "",
      controls: rows.map((row) => box(row.querySelector(".ui-setting-control"))?.left ?? -1),
      more: rows
        .map((row) => box(row.querySelector(".ui-setting-act button")))
        .filter((one): one is Box => one !== null)
        .map((one) => one.right),
      helps: rows.flatMap((row) => {
        const label = box(row.querySelector(".ui-setting-label"));
        const help = box(row.querySelector(".ui-setting-name > .ui-setting-help"));
        const control = box(row.querySelector(".ui-setting-control"));
        return label && help && control ? [{ label, help, control }] : [];
      }),
    };
  });
}

/** The row named `label`: its marks, and its ⋯ button's id, given one for the walk. */
async function rowOf(label: string) {
  return browser.execute((wanted: string) => {
    const name = [...document.querySelectorAll<HTMLElement>(".ui-setting-label")].find(
      (one) => one.textContent?.trim() === wanted,
    );
    const row = name?.closest(".ui-setting-row");
    const more = row?.querySelector<HTMLElement>(".ui-setting-act button");
    if (more) more.id ||= `more-${wanted.replace(/\W+/g, "-")}`;
    return {
      drawn: row !== undefined && row !== null,
      setHere: row?.querySelector('[role="img"][aria-label="Set at this level"]') != null,
      deprecated:
        [...(row?.querySelectorAll('.ui-setting-title [data-scope="badge"]') ?? [])].some(
          (one) => one.textContent === "deprecated",
        ) ?? false,
      more: more?.id ?? "",
    };
  }, label);
}

/** The open menu's rows, each its words and whether it is out of reach. */
async function menuRows() {
  return browser.execute(() =>
    [...document.querySelectorAll('[role="menu"] [role^="menuitem"]')].map((one) => ({
      said: (one.querySelector('[data-part="label"]') ?? one).textContent?.trim() ?? "",
      off: one.getAttribute("aria-disabled") === "true",
    })),
  );
}

/** The page's folded groups: each trigger's words, whether it is open, and whether its chevron
 *  comes after its words. */
async function folds() {
  return browser.execute(() =>
    [
      ...document.querySelectorAll<HTMLElement>(
        '.ui-setting-fold > [data-scope="disclosure"][data-part="trigger"] [aria-expanded]',
      ),
    ].map((one) => {
      const twist = one.querySelector('[data-part="twist"]');
      const words = one.textContent?.trim() ?? "";
      return {
        words,
        open: one.getAttribute("aria-expanded") === "true",
        twistAfter:
          twist !== null &&
          twist.getBoundingClientRect().left >
            (one.querySelector('[data-part="label"]') ?? one).getBoundingClientRect().left,
      };
    }),
  );
}

/** Opens the folded group whose trigger starts with `title`. */
async function unfold(title: string): Promise<void> {
  await browser.execute((wanted: string) => {
    const fold = [
      ...document.querySelectorAll<HTMLElement>(
        '.ui-setting-fold > [data-scope="disclosure"][data-part="trigger"] [aria-expanded]',
      ),
    ].find((one) => one.textContent?.trim().startsWith(`${wanted} · `));
    if (fold?.getAttribute("aria-expanded") === "false") fold.click();
  }, title);
  await browser.waitUntil(
    async () => (await folds()).some((one) => one.words.startsWith(`${title} · `) && one.open),
    { timeout: 10_000, timeoutMsg: `${title} never opened` },
  );
}

/** Opens the ⋯ menu of the button `id`: pressed in the page, the keyboard on it, again until its
 *  menu is up (a press that lands while the page redraws opens none, as `studio.ts` says). */
async function openMore(id: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute((more: string) => {
        if (document.querySelector('[role="menu"]') !== null) return true;
        const one = document.getElementById(more);
        one?.focus();
        one?.click();
        return false;
      }, id),
    { timeout: 20_000, interval: 500, timeoutMsg: "the ⋯ menu never opened" },
  );
}

async function menuOpen(): Promise<boolean> {
  return browser.execute(() => document.querySelector('[role="menu"]') !== null);
}

describe("a Settings page, the aligned form, in the real WebView", () => {
  // At 1280×800, both sidebars open, where the project rail hides itself to leave the work its
  // room (`rail.ts`, #3077): measured with the rail as the window draws it.
  before(() => windowAt(WINDOW_1280));
  after(() => theWindowHoldsTheRail());

  closesTheTabsItOpens();

  it("draws the side column, and the page's header naming its file", async () => {
    await openProjectSettings();
    await $(SETTINGS_GROUPS).$("button=Saving").click();
    await browser.waitUntil(async () => (await rowOf(SET)).drawn, {
      timeout: 20_000,
      timeoutMsg: `Saving never drew ${SET}`,
    });
    const { side, whole, switcher, heading, file, edit } = await laidOut();
    if (!side || !whole) throw new Error("the side column was not drawn");

    expect(switcher).toBe(true);
    // 236 px (R37) in a pane of 944 px or more; narrower, a quarter of it, never under 180 px
    // (#1859): a pixel either way for rounding.
    const wanted = Math.min(236, Math.max(180, whole.width / 4));
    expect(Math.abs(side.width - wanted)).toBeLessThanOrEqual(1);
    expect(heading).toBe("Saving");
    expect(file).toBe("charter.toml");
    expect(edit).toBe("Edit as TOML");
  });

  it("starts every control at one x, ends every row at one ⋯, and puts help under its label", async () => {
    const { controls, more, helps } = await laidOut();

    expect(controls.length).toBeGreaterThan(2);
    expect(Math.max(...controls) - Math.min(...controls)).toBeLessThanOrEqual(1);
    expect(more.length).toBeGreaterThan(2);
    expect(Math.max(...more) - Math.min(...more)).toBeLessThanOrEqual(1);
    expect(helps.length).toBeGreaterThan(2);
    for (const { label, help, control } of helps) {
      expect(help.top).toBeGreaterThanOrEqual(label.bottom - 1);
      expect(Math.abs(help.left - label.left)).toBeLessThanOrEqual(8);
      expect(help.right).toBeLessThanOrEqual(control.left);
    }
  });

  it("folds Branches and Deprecated, each with its count, the chevron after it", async () => {
    const drawn = await folds();

    expect(drawn.map(({ words, open }) => ({ words, open }))).toEqual([
      { words: "Branches · 2", open: false },
      { words: "Deprecated · 1", open: false },
    ]);
    expect(drawn.every((one) => one.twistAfter)).toBe(true);
  });

  it("marks a value set here and a deprecated key, and offers its Reset and Copy key", async () => {
    await unfold("Deprecated");
    const set = await rowOf(SET);
    expect(set.setHere).toBe(true);
    expect(set.deprecated).toBe(true);

    await openMore(set.more);
    const rows = await menuRows();
    expect(rows[0]).toEqual({ said: "Reset", off: false });
    expect(rows.at(-1)).toEqual({ said: "Copy key memory.share", off: false });

    await browser.keys(["Escape"]);
    await browser.waitUntil(async () => !(await menuOpen()), {
      timeout: 10_000,
      timeoutMsg: "Escape never closed the ⋯ menu",
    });
    expect(await browser.execute(() => document.activeElement?.id ?? "")).toBe(set.more);
  });

  it("offers Reset out of reach on a value not set here", async () => {
    await unfold("Branches");
    const unset = await rowOf(UNSET);
    expect(unset.setHere).toBe(false);

    await openMore(unset.more);
    expect((await menuRows())[0]).toEqual({ said: "Reset", off: true });
    await browser.keys(["Escape"]);
    await browser.waitUntil(async () => !(await menuOpen()), {
      timeout: 10_000,
      timeoutMsg: "Escape never closed the ⋯ menu",
    });
  });

  it("opens the file's text from the header's Edit as TOML", async () => {
    await $(".ui-setting-where").$("button=Edit as TOML").click();

    await $('[role="textbox"][aria-label="charter.toml, as TOML"]').waitForDisplayed({
      timeout: 20_000,
    });
  });

  it("folds a group of pages from its toggle, apart from its page, and opens it again", async () => {
    const toggle = async () =>
      browser.execute(() => {
        const one = document.querySelector<HTMLElement>(
          'nav[aria-label="Groups"] [data-part="toggle"]',
        );
        if (!one) return null;
        const row = one.closest("li");
        let pages = 0;
        for (let next = row?.nextElementSibling; next; next = next.nextElementSibling) {
          if (next.querySelector("[data-sub]") === null) break;
          pages += 1;
        }
        return {
          expanded: one.getAttribute("aria-expanded"),
          label: one.getAttribute("aria-label"),
          pages,
        };
      });
    const before = await toggle();
    if (before === null) throw new Error("no group of pages in the nav");
    expect(before.expanded).toBe("true");
    expect(before.pages).toBeGreaterThan(0);

    await browser.execute(() =>
      document.querySelector<HTMLElement>('nav[aria-label="Groups"] [data-part="toggle"]')?.click(),
    );
    await browser.waitUntil(async () => (await toggle())?.expanded === "false", {
      timeout: 10_000,
      timeoutMsg: "the group never folded",
    });
    expect((await toggle())?.pages).toBe(0);

    await browser.execute(() =>
      document.querySelector<HTMLElement>('nav[aria-label="Groups"] [data-part="toggle"]')?.click(),
    );
    await browser.waitUntil(async () => (await toggle())?.expanded === "true", {
      timeout: 10_000,
      timeoutMsg: "the group never opened again",
    });
    expect((await toggle())?.pages).toBe(before.pages);
  });
});
