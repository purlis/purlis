// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { closesTheTabsItOpens } from "../reach.js";
import { SETTINGS_GROUPS, openProjectSettings } from "../settingsTab.js";
import { theWindowHoldsTheRail, WINDOW_1280, windowAt } from "../rail.js";

/**
 * **A settings row is one line at one height, in the real WebView** (DS-8, #1840; spec #1832's
 * seam 3): a select and its Reset come out the same height, side by side, with the row's label
 * on that line; and the group says once where its values are kept, with no line under any row —
 * not even a field's own hint, which is its label's tooltip (D3, D-1840-2).
 *
 * What jsdom cannot say: how tall WebKit draws the select's trigger and a `<button>` once the
 * primitives' classes reach them. Ranges, never exact pixels: the control height is 28px at the
 * default text size, and a text size step or a font's rounding moves it a little.
 * `SettingRow.test.tsx` holds what the cascade gives each one.
 *
 * The row is Saving's `[memory] share (deprecated)`: the fixture project's `charter.toml` sets
 * it, so it has a Reset. **Nothing is written**: the spec only measures, and closes the Settings
 * tab it opened.
 */

const LABEL = "[memory] share (deprecated)";

type Box = { top: number; bottom: number; left: number; right: number; height: number };

/** The row's label, its select and its Reset, measured; and what its group says. */
async function measured() {
  return browser.execute((label: string) => {
    const box = (el: Element | null | undefined): Box | null => {
      if (!el) return null;
      const { top, bottom, left, right, height } = el.getBoundingClientRect();
      return { top, bottom, left, right, height };
    };
    const name = [...document.querySelectorAll<HTMLLabelElement>("label.ui-setting-label")].find(
      (one) => one.textContent?.trim() === label,
    );
    const row = name?.closest(".ui-setting-row");
    const select = name ? document.getElementById(name.htmlFor) : null;
    const reset = [...(row?.querySelectorAll("button") ?? [])].find(
      (one) => one.textContent?.trim() === "Reset",
    );
    const group = row?.closest("section");
    return {
      label: box(name),
      select: box(select),
      selectTag: select?.tagName ?? "",
      reset: box(reset),
      kept: (group?.textContent?.match(/Kept in/g) ?? []).length,
      lines: [...(group?.querySelectorAll(".ui-setting-row p:not(.sr-only)") ?? [])]
        .map((one) => one.textContent ?? "")
        .filter((text) => /^From |Kept in|Not set at this level/.test(text)),
      // Every paragraph drawn under a row: none, since a field's hint is its label's tooltip.
      rowHelp: [...(group?.querySelectorAll(".ui-setting-row p:not(.sr-only)") ?? [])]
        .filter((one) => !one.closest('[role="alert"]'))
        .map((one) => one.textContent ?? ""),
    };
  }, LABEL);
}

describe("a settings row, in the real WebView", () => {
  // At 1280×800, both sidebars open, where the project rail hides itself to leave the work its
  // room (`rail.ts`, #3077): measured with the rail as the window draws it.
  before(() => windowAt(WINDOW_1280));
  after(() => theWindowHoldsTheRail());

  closesTheTabsItOpens();

  it("draws a select and its Reset one height, side by side, on the label's line", async () => {
    await openProjectSettings();
    await $(SETTINGS_GROUPS).$("button=Saving").click();
    await browser.waitUntil(async () => (await measured()).reset !== null, {
      timeout: 20_000,
      timeoutMsg: `Saving never drew ${LABEL} with its Reset`,
    });
    const { label, select, selectTag, reset } = await measured();
    if (!label || !select || !reset) throw new Error("the row was not drawn whole");

    // The window's one select, the select family's on React Aria's `Select` (#1860, #2906): a
    // button that says what it holds.
    expect(selectTag).toBe("BUTTON");
    // One height: the control height, give or take a pixel of rounding.
    expect(Math.abs(select.height - reset.height)).toBeLessThanOrEqual(1);
    expect(select.height).toBeGreaterThanOrEqual(24);
    expect(select.height).toBeLessThanOrEqual(34);
    // Side by side on one line: the same top and bottom, Reset after the select.
    expect(Math.abs(select.top - reset.top)).toBeLessThanOrEqual(1);
    expect(reset.left).toBeGreaterThanOrEqual(select.right);
    // The label's middle is within the controls' line.
    const middle = (label.top + label.bottom) / 2;
    expect(middle).toBeGreaterThanOrEqual(select.top);
    expect(middle).toBeLessThanOrEqual(select.bottom);
  });

  it("says once, in the group, where its values are kept, and draws no help under any row", async () => {
    const { kept, lines, rowHelp } = await measured();

    expect(kept).toBe(1);
    expect(lines).toEqual([]);
    expect(rowHelp).toEqual([]);
  });
});
