// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { fitCounts, fitWays } from "./wholeWays";

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

/**
 * **A breadcrumb's line, laid out by hand** (#1488, #3144): the breadcrumb, Brief, the ways and
 * the harness's name, in a line `width` wide. Brief and the harness are at the padding the
 * stylesheet cuts them to ({@link PADDED}), and nothing at all once set aside; a way is
 * {@link WAY} wide while drawn. The breadcrumb takes what the others leave, and its state needs
 * {@link STATE_NEEDS} of it from its left edge.
 */
const PADDED = 10;
const WAY = 40;
const STATE_NEEDS = 148;

function crumbLine(width: number) {
  const line = document.createElement("div");
  line.className = "pane-chips";
  const crumbs = document.createElement("nav");
  crumbs.className = "pane-crumbs";
  const state = document.createElement("span");
  state.className = "shown-state";
  crumbs.append(state);
  const brief = document.createElement("button");
  brief.className = "pane-brief";
  const group = document.createElement("span");
  group.className = "pane-task-ends";
  const ways = ["Stop", "Close now"].map((words) => {
    const way = document.createElement("button");
    way.className = "task-end";
    way.textContent = words;
    group.append(way);
    return way;
  });
  const harness = document.createElement("span");
  harness.className = "pane-harness";
  line.append(crumbs, brief, group, harness);
  document.body.append(line);

  const away = (el: HTMLElement) => el.dataset.away !== undefined;
  const groupWidth = () => ways.filter((one) => !away(one)).length * WAY;
  const widthOf = (el: HTMLElement) => (el === group ? groupWidth() : away(el) ? 0 : PADDED);
  const rest = [brief, group, harness];
  const crumbsWidth = () => width - rest.reduce((sum, one) => sum + widthOf(one), 0);
  const rect = (left: number, w: number) => ({ left, right: left + w, width: w }) as DOMRect;
  line.getBoundingClientRect = () => rect(0, width);
  crumbs.getBoundingClientRect = () => rect(0, crumbsWidth());
  state.getBoundingClientRect = () => rect(STATE_NEEDS - 128, 128);
  let left = 0;
  const leftOf = (el: HTMLElement) => {
    left = crumbsWidth();
    for (const one of rest) {
      if (one === el) return left;
      left += widthOf(one);
    }
    return left;
  };
  for (const one of rest) one.getBoundingClientRect = () => rect(leftOf(one), widthOf(one));
  ways.forEach((way, at) => {
    way.getBoundingClientRect = () => rect(leftOf(group) + at * WAY, away(way) ? 0 : WAY);
  });
  return { group, ways, brief, harness };
}

describe("the ways to end a task on a breadcrumb's line, as the pane narrows", () => {
  it("draws both ways and every item where the line holds them", () => {
    const { group, ways, brief, harness } = crumbLine(STATE_NEEDS + 2 * WAY + 2 * PADDED);
    fitWays(group);
    expect(drawn(ways)).toHaveLength(2);
    expect(drawn([brief, harness])).toHaveLength(2);
  });

  it("gives up the ways before the padding of the line's other items", () => {
    const { group, ways, brief, harness } = crumbLine(STATE_NEEDS + 2 * PADDED + 5);
    fitWays(group);
    expect(drawn(ways)).toEqual([]);
    expect(drawn([brief, harness])).toHaveLength(2);
  });

  it("then gives up that padding, the last first, so the state stays whole (#3144)", () => {
    const one = crumbLine(STATE_NEEDS + PADDED + 2);
    fitWays(one.group);
    expect(drawn(one.ways)).toEqual([]);
    expect(drawn([one.brief, one.harness])).toEqual([one.brief]);

    document.body.replaceChildren();
    const none = crumbLine(STATE_NEEDS - 5);
    fitWays(none.group);
    expect(drawn([none.brief, none.harness])).toEqual([]);
  });

  it("brings everything back with the room", () => {
    const narrow = crumbLine(STATE_NEEDS);
    fitWays(narrow.group);
    expect(narrow.harness.dataset.away).toBe("");
    document.body.replaceChildren();
    const wide = crumbLine(STATE_NEEDS + 2 * WAY + 2 * PADDED);
    wide.harness.dataset.away = "";
    wide.brief.dataset.away = "";
    fitWays(wide.group);
    expect(drawn([wide.brief, wide.harness])).toHaveLength(2);
  });
});
