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
 * - The side column, titled Settings, beside the page, which scrolls on its own (R37, R38).
 * - The page's header names its file as code, with Edit as TOML after it (R48, R49).
 * - Every control starts at one x and every row's ⋯ stands at one x; each row's help is a line
 *   under its label, in the label's column (R50, R52, R54, R56).
 * - `[memory] share`, which the fixture project's `charter.toml` sets, carries the set-here mark
 *   and the amber deprecated badge (R53, R62); its ⋯ offers Reset in reach and Copy key, and
 *   Escape closes it on its button (R57, R59). A row not set here offers Reset out of reach
 *   (R58).
 * - Edit as TOML in the header opens the file's text (SE-19).
 *
 * What jsdom cannot say: where WebKit lays these out. Ranges, never exact pixels.
 * `SettingsPage.test.tsx` holds the behaviour. **Nothing is written**: the spec opens, measures
 * and closes, and closes the Settings tab it opened.
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
    const rows = [...(page?.querySelectorAll<HTMLElement>(".ui-setting-row") ?? [])];
    return {
      side: box(side),
      title: side?.querySelector(".ui-settings-title")?.textContent ?? "",
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
    const { side, title, heading, file, edit } = await laidOut();
    if (!side) throw new Error("the side column was not drawn");

    expect(title).toBe("Settings");
    // 236 px at the default text size, a pixel either way for rounding (R37).
    expect(Math.abs(side.width - 236)).toBeLessThanOrEqual(1);
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

  it("marks a value set here and a deprecated key, and offers its Reset and Copy key", async () => {
    const set = await rowOf(SET);
    expect(set.setHere).toBe(true);
    expect(set.deprecated).toBe(true);

    await $(`#${set.more}`).click();
    await browser.waitUntil(menuOpen, { timeout: 10_000, timeoutMsg: "the ⋯ menu never opened" });
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
    const unset = await rowOf(UNSET);
    expect(unset.setHere).toBe(false);

    await $(`#${unset.more}`).click();
    await browser.waitUntil(menuOpen, { timeout: 10_000, timeoutMsg: "the ⋯ menu never opened" });
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
});
