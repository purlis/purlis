// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { capacity, fitting, LEAST, LEAST_CHIP, LEAST_TUNED_AT, leastAt, pinnedFirst } from "./fits";
import { DEFAULT_TEXT } from "./textDefaults";

/**
 * The arithmetic that decides what a strip draws.
 *
 * **This is the part `panes.e2e.ts` cannot ask and jsdom cannot answer.** A width is a fact
 * about a laid-out window, so a unit test can only assert what the app does with a width it is
 * given — and that is exactly what this file is for. The other half, that a real WebView gives
 * the strip a width at all and that the tabs in it really do not overflow, is a scenario and
 * lives there.
 */

/** Tabs by name, which is all this cares about. */
const four = ["a", "b", "c", "d"];

describe("how many tabs fit", () => {
  it("is the width over the floor", () => {
    expect(capacity(400, 100)).toBe(4);
    expect(capacity(399, 100)).toBe(3);
  });

  it("is everything when nobody has said how wide the strip is", () => {
    // Zero is the absence of a measurement, not a strip with no room — and every environment
    // with no layout answers zero, including jsdom and the first frame of a real one.
    expect(capacity(0, 100)).toBe(Infinity);
  });

  it("is never zero, however narrow the window is", () => {
    // One squeezed tab is a strip. No tabs at all is a window that looks like it has lost
    // the operator's chats.
    expect(capacity(40, 100)).toBe(1);
  });
});

describe("what a strip draws", () => {
  it("draws everything when everything fits", () => {
    expect(fitting(four, "a", 4 * 100, 100)).toEqual({ shown: four, hidden: [] });
  });

  it("draws a prefix of the order and hides the rest", () => {
    expect(fitting(four, "a", 2 * 100, 100)).toEqual({ shown: ["a", "b"], hidden: ["c", "d"] });
  });

  it("never re-orders what it draws", () => {
    // ADR 0039's first rule. A tab that moves under the cursor breaks aiming, and a collapse
    // is the one change most likely to introduce a sort nobody asked for.
    const { shown, hidden } = fitting(four, "d", 2 * 100, 100);
    expect([...shown, ...hidden].sort()).toEqual([...four].sort());
    expect(shown).toEqual([...shown].sort((one, other) => four.indexOf(one) - four.indexOf(other)));
  });

  it("always draws the selected tab, in the place the order gives it", () => {
    // The promise the scroller kept with `scrollIntoView`. It takes the place of the last tab
    // that fits and stays where the order puts it — a tab that jumped to the right-hand edge
    // when it was picked would be the moving target the record refuses.
    expect(fitting(four, "d", 2 * 100, 100)).toEqual({ shown: ["a", "d"], hidden: ["b", "c"] });
  });

  it("draws the selected tab even with room for one", () => {
    expect(fitting(four, "c", 100, 100)).toEqual({ shown: ["c"], hidden: ["a", "b", "d"] });
  });

  it("hides nothing when nothing is selected and everything fits", () => {
    expect(fitting(four, undefined, 0, 100)).toEqual({ shown: four, hidden: [] });
  });

  it("collapses without a selected tab too", () => {
    // A workspace strip on a plane charter has not read yet has no focused workspace, and a
    // project strip can be drawn while nothing is in front.
    expect(fitting(four, undefined, 2 * 100, 100)).toEqual({
      shown: ["a", "b"],
      hidden: ["c", "d"],
    });
  });

  it("ignores a selected tab that is not on this strip", () => {
    // The chat strip shows one workspace's chats (ADR 0036), so the tab in front can belong
    // to another strip entirely for the frame between a close and the focus following it.
    expect(fitting(four, "z", 2 * 100, 100)).toEqual({ shown: ["a", "b"], hidden: ["c", "d"] });
  });

  it("gives every hidden tab back exactly once", () => {
    // What makes the menu reachable-by-construction: nothing may be dropped between the two
    // lists, because a tab in neither is a chat with no way to it at all.
    const many = Array.from({ length: 50 }, (_, at) => at + 1);
    const { shown, hidden } = fitting(many, 50, 8 * LEAST.chat, LEAST.chat);
    expect(shown).toHaveLength(8);
    expect([...shown, ...hidden].sort((one: number, other: number) => one - other)).toEqual(many);
  });
});

describe("a strip holding chips (SI-8f)", () => {
  // Two chats wrapping up, at the strip's left edge where `tabs.tabsIn` puts them, and four
  // tabs after them.
  const strip = ["x", "y", "a", "b", "c", "d"];
  const chips = { is: (one: string) => one === "x" || one === "y", width: LEAST_CHIP };

  it("shows as many tabs as the room allows, each chip at its own width", () => {
    const room = 2 * LEAST_CHIP + 3 * LEAST.chat;
    expect(fitting(strip, "a", room, LEAST.chat, chips)).toEqual({
      shown: ["x", "y", "a", "b", "c"],
      hidden: ["d"],
    });
  });

  it("never overfills: a pixel less is a tab fewer", () => {
    const room = 2 * LEAST_CHIP + 3 * LEAST.chat - 1;
    expect(fitting(strip, "a", room, LEAST.chat, chips).shown).toEqual(["x", "y", "a", "b"]);
  });

  it("gives the selected tab the room of as many chips as it needs", () => {
    // Room for the two chips and no tab: the selected tab is drawn, and a tab is wider than a
    // chip, so both chips make way for it.
    expect(fitting(strip, "c", 2 * LEAST_CHIP, LEAST.chat, chips)).toEqual({
      shown: ["c"],
      hidden: ["x", "y", "a", "b", "d"],
    });
  });
});

describe("a pinned tab never collapses (ADR 0039, amended 2026-11-02)", () => {
  // Two pins, first on the strip where `tabs.tabsIn` puts them, and six tabs after them.
  const strip = ["p", "q", "a", "b", "c", "d", "e", "f"];
  const pins = { is: (one: string) => one === "p" || one === "q", iconWidth: LEAST_CHIP };

  it("draws everything at the floor when everything fits", () => {
    expect(pinnedFirst(strip, "a", 8 * 120, 120, pins)).toEqual({
      shown: strip,
      hidden: [],
      pinsIconOnly: false,
    });
  });

  it("keeps every pin and collapses only the others", () => {
    // Room for five tabs: the two pins and three of the rest.
    expect(pinnedFirst(strip, "a", 5 * 120, 120, pins)).toEqual({
      shown: ["p", "q", "a", "b", "c"],
      hidden: ["d", "e", "f"],
      pinsIconOnly: false,
    });
  });

  it("still draws the front tab, in the place of the last one that fits", () => {
    expect(pinnedFirst(strip, "f", 5 * 120, 120, pins)).toEqual({
      shown: ["p", "q", "a", "b", "f"],
      hidden: ["c", "d", "e"],
      pinsIconOnly: false,
    });
  });

  it("keeps the pins even where the room would hold nothing else", () => {
    // Pins at their floor would take the whole strip, so they go to their icons, and one
    // other tab, the front one, is still drawn.
    const { shown, pinsIconOnly } = pinnedFirst(strip, "e", 2 * 120, 120, pins);
    expect(shown).toEqual(["p", "q", "e"]);
    expect(pinsIconOnly).toBe(true);
  });

  it("goes icon-only only when the pins alone would take more than half the strip", () => {
    // Two pins at 120 are 240: more than half of 470, not more than half of 480.
    expect(pinnedFirst(strip, "a", 470, 120, pins).pinsIconOnly).toBe(true);
    expect(pinnedFirst(strip, "a", 480, 120, pins).pinsIconOnly).toBe(false);
  });

  it("gives the others the room the pins' icons leave", () => {
    // Pins at 40 each leave 470 - 80 = 390: three tabs at 120.
    expect(pinnedFirst(strip, "a", 470, 120, pins).shown).toEqual(["p", "q", "a", "b", "c"]);
  });

  it("is a front pin's own business: a pinned front tab takes no other tab's place", () => {
    expect(pinnedFirst(strip, "q", 5 * 120, 120, pins).shown).toEqual(["p", "q", "a", "b", "c"]);
  });
});

describe("the floor a chat tab fits by", () => {
  it("is 160px at the default text size, room for about fifteen letters of a name (#5341)", () => {
    expect(leastAt(LEAST.chat, DEFAULT_TEXT.window)).toBe(160);
  });

  it("fits seven tabs where 120px fitted nine, the rest behind show-more", () => {
    const many = Array.from({ length: 12 }, (_, at) => at);
    const floor = leastAt(LEAST.chat, DEFAULT_TEXT.window);
    const { shown, hidden } = fitting(many, 11, 9 * 120 + 40, floor);
    expect(shown).toEqual([0, 1, 2, 3, 4, 5, 11]);
    expect(hidden).toEqual([6, 7, 8, 9, 10]);
  });

  it("keeps a pin and the front tab on the strip at the new floor", () => {
    const floor = leastAt(LEAST.chat, DEFAULT_TEXT.window);
    const strip = ["p", "a", "b", "c", "d", "e"];
    const pins = { is: (one: string) => one === "p", iconWidth: LEAST_CHIP };
    expect(pinnedFirst(strip, "e", 3 * floor, floor, pins)).toEqual({
      shown: ["p", "a", "e"],
      hidden: ["b", "c", "d"],
      pinsIconOnly: false,
    });
  });
});

describe("the floors at the window text size in force (charter-app#283)", () => {
  it("are the floors themselves at the size they were tuned at, and scale with the text", () => {
    expect(leastAt(LEAST.chat, LEAST_TUNED_AT)).toBe(LEAST.chat);
    expect(leastAt(LEAST.chat, 26)).toBe(LEAST.chat * 2);
    // So at the new 14px default a tab floor is a step wider than it was at 13px.
    expect(leastAt(LEAST.chat, 14)).toBeGreaterThan(LEAST.chat);
    expect(leastAt(LEAST.workspace, 10)).toBeLessThan(LEAST.workspace);
  });

  it("so a strip draws fewer, wider tabs when the text is bigger", () => {
    const many = Array.from({ length: 20 }, (_, at) => at);
    const width = 8 * LEAST.chat;
    const atDefault = fitting(many, undefined, width, leastAt(LEAST.chat, LEAST_TUNED_AT)).shown;
    const atBigger = fitting(many, undefined, width, leastAt(LEAST.chat, 21)).shown;
    expect(atBigger.length).toBeLessThan(atDefault.length);
  });
});
