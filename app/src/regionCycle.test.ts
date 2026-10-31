// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { DEFAULT_ARRANGEMENT, panelToggled, type Arrangement } from "./regions";
import { cycleOf, f6Of, stepped, steppedWithCard } from "./regionCycle";

const away = (arrangement: Arrangement, ...ids: string[]) =>
  arrangement.map((one) => (ids.includes(one.id) ? { ...one, collapsed: true } : one));

describe("the regions F6 cycles through (P-13)", () => {
  it("are left sidebar, editor, right sidebar, bottom-left, bottom-right", () => {
    expect(cycleOf(DEFAULT_ARRANGEMENT)).toEqual([
      "left",
      "centre",
      "right",
      "panel-left",
      "panel-right",
    ]);
  });

  it("skip a region that is away", () => {
    expect(cycleOf(away(DEFAULT_ARRANGEMENT, "navigation", "panel-right"))).toEqual([
      "centre",
      "right",
      "panel-left",
    ]);
    // The Panel hidden by ⌘J, and the right sidebar away: the editor is always there.
    expect(cycleOf(away(panelToggled(DEFAULT_ARRANGEMENT), "aside"))).toEqual(["left", "centre"]);
  });
});

describe("a press of F6", () => {
  const all = cycleOf(DEFAULT_ARRANGEMENT);

  it("goes to the next region, and from the last back to the first", () => {
    expect(stepped(all, "centre", false)).toBe("right");
    expect(stepped(all, "panel-right", false)).toBe("left");
  });

  it("with Shift goes back, and from the first to the last", () => {
    expect(stepped(all, "right", true)).toBe("centre");
    expect(stepped(all, "left", true)).toBe("panel-right");
  });

  it("from outside every region goes to the first, or with Shift to the last", () => {
    expect(stepped(all, undefined, false)).toBe("left");
    expect(stepped(all, undefined, true)).toBe("panel-right");
  });

  it("from a region that has just gone away goes on from where it was in the order", () => {
    const left = cycleOf(away(DEFAULT_ARRANGEMENT, "aside"));
    expect(stepped(left, "right", false)).toBe("panel-left");
    expect(stepped(left, "right", true)).toBe("centre");
  });
});

describe("the key", () => {
  const key = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);

  it("is F6 forwards and Shift+F6 backwards, and nothing with another modifier", () => {
    expect(f6Of(key({ key: "F6" }))).toEqual({ back: false });
    expect(f6Of(key({ key: "F6", shiftKey: true }))).toEqual({ back: true });
    expect(f6Of(key({ key: "F6", ctrlKey: true }))).toBeUndefined();
    expect(f6Of(key({ key: "F6", metaKey: true }))).toBeUndefined();
    expect(f6Of(key({ key: "F6", altKey: true }))).toBeUndefined();
    expect(f6Of(key({ key: "F7" }))).toBeUndefined();
  });
});

describe("F6 while a chip's card is up (#3648)", () => {
  const cycle = ["left", "centre", "right"] as const;
  it("goes to the card first, from outside every region and round from the last", () => {
    expect(steppedWithCard(cycle, undefined, "out", false)).toBe("card");
    expect(steppedWithCard(cycle, "right", "out", false)).toBe("card");
    expect(steppedWithCard(cycle, "left", "out", false)).toBe("centre");
  });

  it("goes back to it from the first region with Shift", () => {
    expect(steppedWithCard(cycle, "left", "out", true)).toBe("card");
    expect(steppedWithCard(cycle, undefined, "out", true)).toBe("right");
    expect(steppedWithCard(cycle, "centre", "out", true)).toBe("left");
  });

  it("goes on from the card to the first region, or with Shift the last", () => {
    expect(steppedWithCard(cycle, undefined, "in", false)).toBe("left");
    expect(steppedWithCard(cycle, undefined, "in", true)).toBe("right");
  });

  it("is the cycle as it was with no card up", () => {
    expect(steppedWithCard(cycle, "right", "none", false)).toBe("left");
    expect(steppedWithCard(cycle, undefined, "none", false)).toBe("left");
  });
});
