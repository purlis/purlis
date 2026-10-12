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

type Built = { cell: HTMLElement; chip: HTMLElement; counts: HTMLElement; each: HTMLElement[] };

/** A tab's cell `width` wide, wearing a chip of `many` counts. */
function tab(width: number, many: number): Built {
  const cell = document.createElement("div");
  cell.className = "tab";
  const button = document.createElement("button");
  button.setAttribute("role", "tab");
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
  const widthOf = (el: Element): number =>
    el === button ? FLOOR : el === chip ? chipWidth() : el === closer ? CLOSE : 0;
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
