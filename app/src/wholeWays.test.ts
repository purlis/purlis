// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { fitCounts } from "./wholeWays";

/**
 * **What a tab gives up, in order, when its cell is too narrow for its chip** (#1487, #1867).
 *
 * jsdom lays nothing out, so the cell is laid out here by hand, as a flex row lays it out: the
 * tab's button at its floor, then the chip as wide as the counts it draws, then the close. A
 * thing set aside (`data-away`) takes no room. What is asserted is what `fitCounts` sets aside,
 * which is what the stylesheet then hides; how it looks is `tab-chip.e2e.ts`'s and
 * `tab-strip-centre.e2e.ts`'s, in the real WebView.
 */

/** The tab's button at its floor (2rem at the default text), the close, and each count. */
const FLOOR = 26;
const CLOSE = 20;
const COUNT = 30;
/** What of the button is its mark and padding: at its floor, none of its title is drawn. */
const MARK = FLOOR;
/** A title's whole width. */
const TITLE = 80;

type Built = { cell: HTMLElement; chip: HTMLElement; counts: HTMLElement; each: HTMLElement[] };

/** A tab's cell `width` wide, wearing a chip of `many` counts; `titled`, its button holds a
 *  title {@link TITLE} wide, drawn in what the button has past its mark. */
function tab(width: number, many: number, titled = false): Built {
  const cell = document.createElement("div");
  cell.className = "tab";
  const button = document.createElement("button");
  button.setAttribute("role", "tab");
  const title = document.createElement("span");
  title.className = "tab-name";
  title.textContent = "steward 1";
  if (titled) button.append(title);
  const chip = document.createElement("span");
  chip.className = "tab-tasks";
  const counts = document.createElement("button");
  counts.className = "tab-tasks-counts";
  const each = Array.from({ length: many }, () => {
    const count = document.createElement("span");
    count.className = "count";
    counts.append(count);
    return count;
  });
  chip.append(counts);
  const closer = document.createElement("button");
  closer.className = "closer";
  cell.append(button, chip, closer);
  document.body.append(cell);

  const away = (el: HTMLElement) => el.dataset.away !== undefined;
  const chipWidth = () => (away(chip) ? 0 : each.filter((one) => !away(one)).length * COUNT);
  // The button takes what the chip and the close leave, down to its floor, as a flex row gives.
  const buttonWidth = () => Math.max(FLOOR, width - chipWidth() - CLOSE);
  const widthOf = (el: Element): number =>
    el === button ? buttonWidth() : el === chip ? chipWidth() : el === closer ? CLOSE : 0;
  title.getBoundingClientRect = () =>
    rect(MARK, Math.min(TITLE, Math.max(0, buttonWidth() - MARK)));
  const rect = (left: number, w: number) => ({ left, right: left + w, width: w }) as DOMRect;
  cell.getBoundingClientRect = () => rect(0, width);
  for (const child of [button, chip, closer]) {
    child.getBoundingClientRect = () => {
      let left = 0;
      for (const before of [button, chip, closer]) {
        if (before === child) break;
        left += widthOf(before);
      }
      return rect(left, widthOf(child));
    };
  }
  return { cell, chip, counts, each };
}

const drawn = (each: HTMLElement[]) => each.filter((one) => one.dataset.away === undefined);

afterEach(() => {
  document.body.replaceChildren();
});

describe("the chip on a tab, as its cell narrows", () => {
  it("draws every count where the cell holds them all", () => {
    const { chip, counts, each } = tab(FLOOR + 3 * COUNT + CLOSE, 3);
    fitCounts(counts);
    expect(drawn(each)).toHaveLength(3);
    expect(chip.dataset.away).toBeUndefined();
  });

  it("gives up counts from the end, keeping what is still working", () => {
    const { chip, counts, each } = tab(FLOOR + COUNT + CLOSE + 5, 3);
    fitCounts(counts);
    expect(drawn(each)).toEqual([each[0]]);
    expect(chip.dataset.away).toBeUndefined();
  });

  it("gives up the whole chip before the tab's title, where not even one count fits", () => {
    // Narrower than the floor, one count and the close: the centre column at 900 px with both
    // sidebars open (#1867).
    const { chip, counts } = tab(FLOOR + CLOSE + 10, 3);
    fitCounts(counts);
    expect(chip.dataset.away).toBe("");
  });

  it("gives up counts, then the whole chip, where the title would not be drawn at all (#1867)", () => {
    // Every part inside the cell, the button at its floor: its mark, and no letter of its title.
    const wide = tab(FLOOR + 2 * COUNT + CLOSE, 2, true);
    fitCounts(wide.counts);
    expect(drawn(wide.each)).toEqual([wide.each[0]]);
    expect(wide.chip.dataset.away).toBeUndefined();

    document.body.replaceChildren();
    const narrow = tab(FLOOR + COUNT + CLOSE, 2, true);
    fitCounts(narrow.counts);
    expect(narrow.chip.dataset.away).toBe("");
  });

  it("keeps every count beside a title that is drawn, however little of it", () => {
    const { chip, counts, each } = tab(MARK + 2 + 2 * COUNT + CLOSE, 2, true);
    fitCounts(counts);
    expect(drawn(each)).toEqual(each);
    expect(chip.dataset.away).toBeUndefined();
  });

  it("brings the chip back once the cell has room again", () => {
    const narrow = tab(FLOOR + CLOSE + 10, 2);
    fitCounts(narrow.counts);
    expect(narrow.chip.dataset.away).toBe("");

    narrow.cell.getBoundingClientRect = () => ({ left: 0, right: 400, width: 400 }) as DOMRect;
    fitCounts(narrow.counts);
    expect(narrow.chip.dataset.away).toBeUndefined();
    expect(drawn(narrow.each)).toHaveLength(2);
  });
});
