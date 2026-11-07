// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { fitWays } from "./wholeWays";

const drawn = (each: HTMLElement[]) => each.filter((one) => one.dataset.away === undefined);

afterEach(() => {
  document.body.replaceChildren();
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
