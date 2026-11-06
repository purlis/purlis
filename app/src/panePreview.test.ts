// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { type DropContext, dropOn, type Source, type Target } from "./paneDrop";
import { boxesOf, gridIn, planOf, type TerminalCells, wouldBe } from "./panePreview";
import { contentsOf, insertPane, type Content, noTabs, openTab, type Tabs } from "./tabs";

/**
 * **What a drag draws before the drop** (#5291): the panes where the drop model's own result puts
 * them. Plain values in and out; the window only measures the room and writes these boxes down.
 */

const ctx: DropContext = {
  filedIn: (session) => (session > 20 ? "volaticloud" : "ops"),
  name: (content) => (content.kind === "session" ? `chat ${content.session}` : "the empty pane"),
};

const ROOM = { left: 100, top: 50, width: 1000, height: 600 };
const NO_GAPS = { separator: 0, fold: 0 };

const chat = (session: number): Content => ({ kind: "session", session, chat: String(session) });

function paneOf(tabs: Tabs, tab: number, session: number): number {
  const found = contentsOf(tabs, tab).find(
    ({ content }) => content.kind === "session" && content.session === session,
  );
  if (found === undefined) throw new Error(`tab ${tab} shows no chat ${session}`);
  return found.pane;
}

/** Chat 1 left of chat 2, and a second tab behind, with chat 4. */
function two(): { tabs: Tabs; tab: number; one: number; other: number } {
  let tabs = openTab(noTabs(), 1, "1", "steward");
  const tab = tabs.order[0];
  tabs = insertPane(tabs, {
    tab,
    pane: paneOf(tabs, tab, 1),
    direction: "row",
    side: "after",
    content: chat(2),
  });
  return { tabs, tab, one: paneOf(tabs, tab, 1), other: paneOf(tabs, tab, 2) };
}

const onScreenOf = (tabs: Tabs, tab: number) =>
  new Map(contentsOf(tabs, tab).map(({ pane, content }) => [pane, content]));

describe("where each pane of a layout is drawn (#5291)", () => {
  it("gives a split's sides their shares of the room, less the separator between them", () => {
    const { tabs, tab, one, other } = two();
    const boxes = boxesOf(tabs.byId[tab].layout, one, ROOM, { separator: 10, fold: 0 });
    expect(boxes.panes.get(one)).toEqual({ left: 100, top: 50, width: 495, height: 600 });
    expect(boxes.panes.get(other)).toEqual({ left: 605, top: 50, width: 495, height: 600 });
    expect(boxes.rails).toEqual([]);
  });

  it("keeps a split's own share", () => {
    const { tabs, tab, one, other } = two();
    const layout = tabs.byId[tab].layout;
    if (layout.kind !== "split") throw new Error("a split");
    const boxes = boxesOf({ ...layout, share: 300 }, one, ROOM, NO_GAPS);
    expect(boxes.panes.get(one)?.width).toBe(300);
    expect(boxes.panes.get(other)).toEqual({ left: 400, top: 50, width: 700, height: 600 });
  });

  it("folds as the window folds: the side without the focus is a rail, the other has the rest", () => {
    const { tabs, tab, one, other } = two();
    const narrow = { left: 0, top: 0, width: 500, height: 600 };
    const boxes = boxesOf(tabs.byId[tab].layout, one, narrow, { separator: 8, fold: 4 });
    expect(boxes.panes.get(one)).toEqual({ left: 0, top: 0, width: 456, height: 600 });
    expect(boxes.panes.has(other)).toBe(false);
    expect(boxes.rails).toEqual([
      { box: { left: 460, top: 0, width: 40, height: 600 }, as: "rail", panes: [other] },
    ]);
  });
});

describe("what a drop would leave in the tab in front (#5291)", () => {
  const pane = (tabs: Tabs, tab: number, session: number): Source => ({
    kind: "pane",
    tab,
    pane: paneOf(tabs, tab, session),
  });

  it("is the drop model's own answer, for every place", () => {
    const { tabs, tab, one } = two();
    for (const place of ["left", "right", "top", "bottom", "middle"] as const) {
      const to: Target = { kind: "pane", tab, pane: one, place };
      const after = dropOn(tabs, pane(tabs, tab, 2), to, ctx);
      expect(wouldBe(tabs, pane(tabs, tab, 2), to, ctx)?.layout).toEqual(after.byId[tab].layout);
    }
  });

  it("is nothing for a place that is refused, so a preview can't promise what a drop won't do", () => {
    const { tabs, tab, one } = two();
    const to: Target = { kind: "pane", tab, pane: one, place: "left" };
    expect(wouldBe(tabs, pane(tabs, tab, 2), to, ctx, { width: 100, height: 600 })).toBeUndefined();
  });

  it("is nothing over its own place", () => {
    const { tabs, tab, one } = two();
    const to: Target = { kind: "pane", tab, pane: one, place: "left" };
    expect(wouldBe(tabs, pane(tabs, tab, 1), to, ctx)).toBeUndefined();
  });

  it("is nothing on the strip, whose line says where the tab goes while the panes stay put", () => {
    const { tabs, tab } = two();
    expect(wouldBe(tabs, pane(tabs, tab, 2), { kind: "strip", at: 1 }, ctx)).toBeUndefined();
  });

  it("changes nothing in the tabs it was asked of", () => {
    const { tabs, tab, one } = two();
    const before = JSON.stringify(tabs);
    wouldBe(tabs, pane(tabs, tab, 2), { kind: "pane", tab, pane: one, place: "top" }, ctx);
    expect(JSON.stringify(tabs)).toBe(before);
  });
});

describe("what each pane does in the preview (#5291)", () => {
  it("moves the panes on screen to their boxes", () => {
    const { tabs, tab, one, other } = two();
    const source: Source = { kind: "pane", tab, pane: other };
    const to: Target = { kind: "pane", tab, pane: one, place: "left" };
    const after = wouldBe(tabs, source, to, ctx);
    if (after === undefined) throw new Error("it can be dropped");
    const plan = planOf(after, onScreenOf(tabs, tab), ROOM, NO_GAPS);
    expect(plan.frames.get(other)).toEqual({ left: 100, top: 50, width: 500, height: 600 });
    expect(plan.frames.get(one)).toEqual({ left: 600, top: 50, width: 500, height: 600 });
    expect(plan.arriving).toEqual([]);
    expect(plan.gone).toEqual([]);
  });

  it("draws a chat that has no pane yet as one arriving, and room made for it", () => {
    const { tabs, tab, one, other } = two();
    const source: Source = { kind: "chat", session: 7, chat: "7" };
    const to: Target = { kind: "pane", tab, pane: other, place: "bottom" };
    const after = wouldBe(tabs, source, to, ctx);
    if (after === undefined) throw new Error("it can be dropped");
    const plan = planOf(after, onScreenOf(tabs, tab), ROOM, NO_GAPS);
    expect(plan.frames.get(one)).toEqual({ left: 100, top: 50, width: 500, height: 600 });
    expect(plan.frames.get(other)).toEqual({ left: 600, top: 50, width: 500, height: 300 });
    expect(plan.arriving).toEqual([
      {
        pane: expect.any(Number),
        box: { left: 600, top: 350, width: 500, height: 300 },
        content: { kind: "session", session: 7, chat: "7" },
      },
    ]);
  });

  it("draws a chat shown in a pane's place over that pane, which keeps running behind it", () => {
    const { tabs, tab, other } = two();
    const source: Source = { kind: "chat", session: 7, chat: "7" };
    const to: Target = { kind: "pane", tab, pane: other, place: "middle" };
    const after = wouldBe(tabs, source, to, ctx);
    if (after === undefined) throw new Error("it can be dropped");
    const plan = planOf(after, onScreenOf(tabs, tab), ROOM, NO_GAPS);
    expect(plan.frames.has(other)).toBe(false);
    expect(plan.gone).toEqual([other]);
    expect(plan.arriving.map(({ content }) => content)).toEqual([
      { kind: "session", session: 7, chat: "7" },
    ]);
  });

  it("closes up an empty pane that is filled: the pane moved takes its whole box", () => {
    const { tabs, tab, one, other } = two();
    const emptied = insertPane(tabs, {
      tab,
      pane: other,
      direction: "column",
      side: "after",
      content: { kind: "empty", workspace: "ops" },
    });
    const empty = contentsOf(emptied, tab).find(({ content }) => content.kind === "empty");
    if (empty === undefined) throw new Error("an empty pane");
    const source: Source = { kind: "pane", tab, pane: one };
    const to: Target = { kind: "pane", tab, pane: empty.pane, place: "middle" };
    const after = wouldBe(emptied, source, to, ctx);
    if (after === undefined) throw new Error("it can be dropped");
    const plan = planOf(after, onScreenOf(emptied, tab), ROOM, NO_GAPS);
    expect(plan.gone).toEqual([empty.pane]);
    expect(plan.frames.get(other)).toEqual({ left: 100, top: 50, width: 1000, height: 300 });
    expect(plan.frames.get(one)).toEqual({ left: 100, top: 350, width: 1000, height: 300 });
  });

  it("draws a pane the window has no frame for, folded now, as one arriving", () => {
    const { tabs, tab, one, other } = two();
    const source: Source = { kind: "chat", session: 7, chat: "7" };
    const to: Target = { kind: "pane", tab, pane: one, place: "top" };
    const after = wouldBe(tabs, source, to, ctx);
    if (after === undefined) throw new Error("it can be dropped");
    const shown = onScreenOf(tabs, tab);
    shown.delete(other);
    const plan = planOf(after, shown, ROOM, NO_GAPS);
    expect(plan.arriving.map(({ pane }) => pane)).toContain(other);
  });

  it("marks a pane that would fold as gone, and draws its rail", () => {
    const { tabs, tab, one, other } = two();
    const narrow = { left: 0, top: 0, width: 620, height: 600 };
    const source: Source = { kind: "chat", session: 7, chat: "7" };
    const to: Target = { kind: "pane", tab, pane: one, place: "bottom" };
    const after = wouldBe(tabs, source, to, ctx);
    if (after === undefined) throw new Error("it can be dropped");
    const plan = planOf(after, onScreenOf(tabs, tab), narrow, NO_GAPS);
    expect(plan.rails).toHaveLength(0);
    const tight = planOf(
      after,
      onScreenOf(tabs, tab),
      { left: 0, top: 0, width: 500, height: 600 },
      NO_GAPS,
    );
    expect(tight.rails.length).toBeGreaterThan(0);
    expect(tight.gone).toContain(other);
  });
});

describe("the size a terminal will take (#5291)", () => {
  const cells: TerminalCells = {
    width: 8,
    height: 16,
    aroundWidth: 20,
    aroundHeight: 64,
  };

  it("is what fits in the box, less what the pane draws around its terminal", () => {
    expect(gridIn({ width: 820, height: 864 }, cells)).toEqual({ columns: 100, rows: 50 });
  });

  it("rounds down, as a terminal's fit does", () => {
    expect(gridIn({ width: 827, height: 879 }, cells)).toEqual({ columns: 100, rows: 50 });
  });

  it("never says less than the least a terminal has", () => {
    expect(gridIn({ width: 10, height: 10 }, cells)).toEqual({ columns: 2, rows: 1 });
  });
});
