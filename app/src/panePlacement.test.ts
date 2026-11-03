// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { type DropContext, type Source, type Target, judgeDrop } from "./paneDrop";
import {
  groupsOf,
  pickedUpSaid,
  placingDrop,
  stepFrom,
  firstTarget,
  targetsOf,
} from "./panePlacement";
import { contentsOf, insertPane, noTabs, openTab, selectTab, type Tabs } from "./tabs";

/**
 * **Placement mode** (#4764): the keyboard and single-press route for every drag, on plain
 * values. What a target does and why one is refused is `paneDrop.ts`'s, the pointer's own rules;
 * this says which targets there are and how the keys walk them.
 */

const ctx: DropContext = {
  filedIn: () => "ops",
  name: (content) => (content.kind === "session" ? `chat ${content.session}` : "the empty pane"),
};

/** Tab A: chat 1 | chat 3 | chat 5 side by side; tab B: chat 4. Tab A in front. */
function start(): { tabs: Tabs; a: number; b: number; one: number; three: number; five: number } {
  let tabs = openTab(noTabs(), 1, "1", "steward");
  const a = tabs.order[0];
  const one = contentsOf(tabs, a)[0].pane;
  tabs = insertPane(tabs, {
    tab: a,
    pane: one,
    direction: "row",
    side: "after",
    content: { kind: "session", session: 3, chat: "3" },
  });
  const three = contentsOf(tabs, a)[1].pane;
  tabs = insertPane(tabs, {
    tab: a,
    pane: three,
    direction: "row",
    side: "after",
    content: { kind: "session", session: 5, chat: "5" },
  });
  const five = contentsOf(tabs, a)[2].pane;
  tabs = openTab(tabs, 4, "4", "steward");
  const b = tabs.order[1];
  return { tabs: selectTab(tabs, a), a, b, one, three, five };
}

describe("which targets placement mode shows (#4764)", () => {
  it("shows five targets on every other pane of the tab in front, then a New tab on the strip", () => {
    const { tabs, a, three, five, one } = start();
    const source: Source = { kind: "pane", tab: a, pane: one };
    expect(groupsOf(tabs, source)).toEqual([
      { kind: "pane", tab: a, pane: three },
      { kind: "pane", tab: a, pane: five },
      { kind: "strip", at: tabs.order.length },
    ]);
    expect(
      targetsOf({ kind: "pane", tab: a, pane: three }).map((t) => t.kind === "pane" && t.place),
    ).toEqual(["top", "left", "middle", "right", "bottom"]);
  });

  it("shows no targets on the pane a sidebar chat already shows: that pane is what moves", () => {
    const { tabs, a, one, five } = start();
    const source: Source = { kind: "chat", session: 3, chat: "3" };
    expect(groupsOf(tabs, source)).toEqual([
      { kind: "pane", tab: a, pane: one },
      { kind: "pane", tab: a, pane: five },
      { kind: "strip", at: tabs.order.length },
    ]);
  });

  it("shows every pane of the tab in front for a chat no pane shows", () => {
    const { tabs, a, one, three, five } = start();
    const source: Source = { kind: "chat", session: 9, chat: "9" };
    expect(groupsOf(tabs, source).filter((g) => g.kind === "pane")).toEqual([
      { kind: "pane", tab: a, pane: one },
      { kind: "pane", tab: a, pane: three },
      { kind: "pane", tab: a, pane: five },
    ]);
  });

  it("follows the tab brought forward while a pane is held, as a pointer held over a tab does", () => {
    const { tabs, a, b, one } = start();
    const source: Source = { kind: "pane", tab: a, pane: one };
    const behind = selectTab(tabs, b);
    const four = contentsOf(behind, b)[0].pane;
    expect(groupsOf(behind, source)[0]).toEqual({ kind: "pane", tab: b, pane: four });
  });
});

describe("walking the targets with the keys (#4764)", () => {
  const { tabs, a, one, three, five } = start();
  const groups = groupsOf(tabs, { kind: "pane", tab: a, pane: one });
  const at = (pane: number, place: "top" | "left" | "middle" | "right" | "bottom"): Target => ({
    kind: "pane",
    tab: a,
    pane,
    place,
  });
  const strip: Target = { kind: "strip", at: tabs.order.length };

  it("starts on the middle of the first other pane", () => {
    expect(firstTarget(groups)).toEqual(at(three, "middle"));
  });

  it("moves from the middle to the side an arrow points at", () => {
    expect(stepFrom(groups, at(three, "middle"), "ArrowLeft")).toEqual(at(three, "left"));
    expect(stepFrom(groups, at(three, "middle"), "ArrowUp")).toEqual(at(three, "top"));
    expect(stepFrom(groups, at(three, "middle"), "ArrowRight")).toEqual(at(three, "right"));
    expect(stepFrom(groups, at(three, "middle"), "ArrowDown")).toEqual(at(three, "bottom"));
  });

  it("comes back to the middle with the opposite arrow, and stays at a side's own arrow", () => {
    expect(stepFrom(groups, at(three, "left"), "ArrowRight")).toEqual(at(three, "middle"));
    expect(stepFrom(groups, at(three, "top"), "ArrowDown")).toEqual(at(three, "middle"));
    expect(stepFrom(groups, at(three, "left"), "ArrowLeft")).toEqual(at(three, "left"));
  });

  it("goes round the cross from one side to the next with the arrow toward it", () => {
    expect(stepFrom(groups, at(three, "left"), "ArrowUp")).toEqual(at(three, "top"));
    expect(stepFrom(groups, at(three, "bottom"), "ArrowRight")).toEqual(at(three, "right"));
  });

  it("goes to the next pane's middle on Tab, then the strip, then round again", () => {
    expect(stepFrom(groups, at(three, "left"), "Tab")).toEqual(at(five, "middle"));
    expect(stepFrom(groups, at(five, "middle"), "Tab")).toEqual(strip);
    expect(stepFrom(groups, strip, "Tab")).toEqual(at(three, "middle"));
  });

  it("goes back a pane on Shift+Tab", () => {
    expect(stepFrom(groups, at(three, "middle"), "ShiftTab")).toEqual(strip);
    expect(stepFrom(groups, strip, "ShiftTab")).toEqual(at(five, "middle"));
  });

  it("does nothing with an arrow on the strip's one target", () => {
    expect(stepFrom(groups, strip, "ArrowLeft")).toEqual(strip);
  });

  it("starts again on the first target when the target it was on has gone", () => {
    expect(stepFrom(groups, at(99, "left"), "Tab")).toEqual(at(three, "middle"));
  });
});

describe("what a placement says and does (#4764)", () => {
  it("says what was picked up and the keys that place it", () => {
    expect(pickedUpSaid("chat 1")).toBe(
      "Picked up chat 1. Arrow keys choose a side, Tab the next pane, Enter drops, Escape cancels.",
    );
  });

  it("drops by the pointer's own rules: the same verdict and the same tabs", () => {
    const { tabs, a, one, three } = start();
    const source: Source = { kind: "pane", tab: a, pane: one };
    const to: Target = { kind: "pane", tab: a, pane: three, place: "right" };
    const done = placingDrop(tabs, source, to, ctx);
    const verdict = judgeDrop(tabs, source, to, ctx);
    expect(verdict.ok).toBe(true);
    expect(done.said).toBe(verdict.ok && `Dropped. ${verdict.did}`);
    expect(done.tabs).not.toBe(tabs);
    expect(done.tabs.undo?.did).toBe(verdict.ok && verdict.did);
  });

  it("says why a refused target takes nothing, and leaves the tabs as they were", () => {
    const { tabs, a, one, three } = start();
    const source: Source = { kind: "pane", tab: a, pane: one };
    const to: Target = { kind: "pane", tab: a, pane: three, place: "right" };
    const done = placingDrop(tabs, source, to, ctx, { width: 400, height: 600 });
    expect(done.tabs).toBe(tabs);
    expect(done.said).toBe(
      "Not dropped: Too narrow: a pane here would be under about 40 columns. Drop it on a wider pane.",
    );
  });
});
