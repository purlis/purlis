// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  type DropContext,
  dropOn,
  floorAlong,
  judgeDrop,
  PANE_FLOOR,
  placeAt,
  previewOf,
  type Source,
  undone,
} from "./paneDrop";
import {
  type Content,
  contentsOf,
  insertPane,
  type Layout,
  noTabs,
  openTab,
  openView,
  selectTab,
  showIn,
  type Tabs,
  type ViewRef,
} from "./tabs";

/**
 * **Dragging panes: split, swap and move** (#4763), the window's pure layout model. Every drop
 * the prototype's verdict rules (`prototype/pane-drag`, resolved on #4583) and every refusal is
 * here; the window only measures where the pointer is and draws the answer.
 */

const SETTINGS: ViewRef = { from: null, view: "settings", key: "project" };

/** Chats 1 to 9 work in `ops`, chat 21 in `volaticloud`; each is named `chat <n>`. */
const ctx: DropContext = {
  filedIn: (session) => (session > 20 ? "volaticloud" : "ops"),
  name: (content) =>
    content.kind === "session"
      ? `chat ${content.session}`
      : content.kind === "view"
        ? "Settings"
        : "empty pane",
};

/** Big enough for any split. */
const ROOMY = { width: 1600, height: 900 };

/** A tab's tree, short: a chat by its number, `-` for nothing, `v` for a view, a split's
 *  share only when it is not even. */
function shape(layout: Layout): string {
  if (layout.kind === "pane") {
    const c = layout.content;
    return c.kind === "session" ? String(c.session) : c.kind === "empty" ? "-" : "v";
  }
  const [a, b] = layout.children;
  const share = layout.share === undefined || layout.share === 500 ? "" : `@${layout.share}`;
  return `${layout.direction}${share}(${shape(a)} ${shape(b)})`;
}

const shapeOf = (tabs: Tabs, id: number) => shape(tabs.byId[id].layout);
const strip = (tabs: Tabs) => tabs.order.map((id) => shapeOf(tabs, id));
function paneOf(tabs: Tabs, id: number, session: number): number {
  const found = contentsOf(tabs, id).find(
    ({ content }) => content.kind === "session" && content.session === session,
  );
  if (found === undefined) throw new Error(`tab ${id} shows no chat ${session}`);
  return found.pane;
}
const chat = (session: number): Content => ({ kind: "session", session, chat: String(session) });

/** One tab: chat 1, with chat 2 right of it and chat 3 below chat 2. */
function three(): { tabs: Tabs; tab: number } {
  let tabs = openTab(noTabs(), 1, "1", "steward");
  const tab = tabs.order[0];
  tabs = insertPane(tabs, {
    tab,
    pane: paneOf(tabs, tab, 1),
    direction: "row",
    side: "after",
    content: chat(2),
  });
  tabs = insertPane(tabs, {
    tab,
    pane: paneOf(tabs, tab, 2),
    direction: "column",
    side: "after",
    content: chat(3),
  });
  return { tabs, tab };
}

/** Two tabs: chat 1 alone, then chats 4 and 5 side by side, in front. */
function twoTabs(): { tabs: Tabs; first: number; second: number } {
  let tabs = openTab(openTab(noTabs(), 1, "1", "steward"), 4, "4", "steward");
  const [first, second] = tabs.order;
  tabs = insertPane(tabs, {
    tab: second,
    pane: paneOf(tabs, second, 4),
    direction: "row",
    side: "after",
    content: chat(5),
  });
  return { tabs, first, second };
}

const paneSource = (tabs: Tabs, tab: number, session: number): Source => ({
  kind: "pane",
  tab,
  pane: paneOf(tabs, tab, session),
});

describe("a pane dropped on another pane's edge splits on that side (#4763)", () => {
  it.each([
    ["left", "row(row(3 1) 2)", "chat 3 left of chat 1."],
    ["top", "row(column(3 1) 2)", "chat 3 above chat 1."],
    ["right", "row(row(1 3) 2)", "chat 3 right of chat 1."],
    ["bottom", "row(column(1 3) 2)", "chat 3 below chat 1."],
  ] as const)("on the %s edge", (place, after, says) => {
    const { tabs, tab } = three();
    const from = paneSource(tabs, tab, 3);
    const to = { kind: "pane", tab, pane: paneOf(tabs, tab, 1), place } as const;
    expect(judgeDrop(tabs, from, to, ctx, ROOMY)).toEqual({ ok: true, says });
    expect(shapeOf(dropOn(tabs, from, to, ctx), tab)).toBe(after);
  });

  it("puts the source first for left and top, second for right and bottom, at an even share", () => {
    const { tabs, tab } = three();
    const from = paneSource(tabs, tab, 1);
    const left = dropOn(
      tabs,
      from,
      { kind: "pane", tab, pane: paneOf(tabs, tab, 3), place: "left" },
      ctx,
    );
    expect(shapeOf(left, tab)).toBe("column(2 row(1 3))");
    const below = dropOn(
      tabs,
      from,
      { kind: "pane", tab, pane: paneOf(tabs, tab, 2), place: "bottom" },
      ctx,
    );
    expect(shapeOf(below, tab)).toBe("column(column(2 1) 3)");
  });

  it("keeps the moved pane's number, so its terminal is the same one, and focuses it", () => {
    const { tabs, tab } = three();
    const moved = paneOf(tabs, tab, 3);
    const after = dropOn(
      tabs,
      { kind: "pane", tab, pane: moved },
      { kind: "pane", tab, pane: paneOf(tabs, tab, 1), place: "left" },
      ctx,
    );
    expect(paneOf(after, tab, 3)).toBe(moved);
    expect(after.byId[tab].focused).toBe(moved);
  });

  it("moves a pane into another tab, which comes forward, and leaves its sibling the split", () => {
    const { tabs, first, second } = twoTabs();
    const after = dropOn(
      tabs,
      paneSource(tabs, second, 5),
      { kind: "pane", tab: first, pane: paneOf(tabs, first, 1), place: "right" },
      ctx,
    );
    expect(strip(after)).toEqual(["row(1 5)", "4"]);
    expect(after.inFront).toBe(first);
  });

  it("closes the tab a pane's last chat left, when another tab is on its strip", () => {
    const { tabs, first, second } = twoTabs();
    const after = dropOn(
      tabs,
      paneSource(tabs, first, 1),
      { kind: "pane", tab: second, pane: paneOf(tabs, second, 5), place: "bottom" },
      ctx,
    );
    expect(after.order).toEqual([second]);
    expect(shapeOf(after, second)).toBe("row(4 column(5 1))");
  });

  it("keeps an empty pane in the strip's last tab when its last chat leaves it", () => {
    let tabs = openTab(noTabs(), 21, "21", "devops");
    const lone = tabs.order[0];
    tabs = openTab(tabs, 1, "1", "steward");
    const ops = tabs.order[1];
    const after = dropOn(
      tabs,
      paneSource(tabs, lone, 21),
      { kind: "pane", tab: ops, pane: paneOf(tabs, ops, 1), place: "right" },
      ctx,
    );
    expect(strip(after)).toEqual(["-", "row(1 21)"]);
    expect(contentsOf(after, lone)[0].content).toEqual({
      kind: "empty",
      workspace: "volaticloud",
    });
  });

  it("splits beside a view or an empty pane as beside a chat", () => {
    let tabs = openView(openTab(noTabs(), 1, "1", "steward"), SETTINGS, "Settings", "ops");
    const [chatTab, viewTab] = tabs.order;
    const viewPane = contentsOf(tabs, viewTab)[0].pane;
    tabs = dropOn(
      tabs,
      paneSource(tabs, chatTab, 1),
      { kind: "pane", tab: viewTab, pane: viewPane, place: "left" },
      ctx,
    );
    expect(strip(tabs)).toEqual(["row(1 v)"]);
    tabs = showIn(tabs, viewTab, viewPane, { kind: "empty", workspace: "ops" });
    const after = dropOn(
      tabs,
      { kind: "chat", session: 7, chat: "7" },
      { kind: "pane", tab: viewTab, pane: viewPane, place: "bottom" },
      ctx,
    );
    expect(strip(after)).toEqual(["row(1 column(- 7))"]);
  });
});

describe("a pane dropped in another pane's middle (#4763)", () => {
  it("swaps the two, each keeping its own pane", () => {
    const { tabs, tab } = three();
    const one = paneOf(tabs, tab, 1);
    const from = paneSource(tabs, tab, 3);
    const to = { kind: "pane", tab, pane: one, place: "middle" } as const;
    expect(judgeDrop(tabs, from, to, ctx, ROOMY)).toEqual({
      ok: true,
      says: "Swap chat 3 and chat 1.",
    });
    const after = dropOn(tabs, from, to, ctx);
    expect(shapeOf(after, tab)).toBe("row(3 column(2 1))");
    expect(paneOf(after, tab, 1)).toBe(one);
    expect(after.byId[tab].focused).toBe(paneOf(after, tab, 3));
  });

  it("swaps across two tabs", () => {
    const { tabs, first, second } = twoTabs();
    const after = dropOn(
      tabs,
      paneSource(tabs, second, 5),
      { kind: "pane", tab: first, pane: paneOf(tabs, first, 1), place: "middle" },
      ctx,
    );
    expect(strip(after)).toEqual(["5", "row(4 1)"]);
    expect(after.inFront).toBe(first);
  });

  it("fills an empty pane, and the source's old pane closes", () => {
    const made = three();
    const tab = made.tab;
    const two = paneOf(made.tabs, tab, 2);
    const tabs = showIn(made.tabs, tab, two, { kind: "empty", workspace: "ops" });
    const from = paneSource(tabs, tab, 1);
    const to = { kind: "pane", tab, pane: two, place: "middle" } as const;
    expect(judgeDrop(tabs, from, to, ctx, ROOMY)).toEqual({
      ok: true,
      says: "Put chat 1 in this empty pane.",
    });
    expect(shapeOf(dropOn(tabs, from, to, ctx), tab)).toBe("column(1 3)");
  });
});

describe("a sidebar chat dropped on a pane (#4763)", () => {
  it("replaces what the pane shows in its middle; the chat it showed keeps running", () => {
    const { tabs, tab } = three();
    const from: Source = { kind: "chat", session: 7, chat: "7" };
    const to = { kind: "pane", tab, pane: paneOf(tabs, tab, 2), place: "middle" } as const;
    expect(judgeDrop(tabs, from, to, ctx, ROOMY)).toEqual({
      ok: true,
      says: "Show chat 7 here. chat 2 keeps running in the sidebar.",
    });
    expect(shapeOf(dropOn(tabs, from, to, ctx), tab)).toBe("row(1 column(7 3))");
  });

  it("splits beside a pane on its edge", () => {
    const { tabs, tab } = three();
    const after = dropOn(
      tabs,
      { kind: "chat", session: 7, chat: "7" },
      { kind: "pane", tab, pane: paneOf(tabs, tab, 3), place: "top" },
      ctx,
    );
    expect(shapeOf(after, tab)).toBe("row(1 column(2 column(7 3)))");
    expect(after.byId[tab].focused).toBe(paneOf(after, tab, 7));
  });

  it("moves a chat already on screen rather than showing it twice", () => {
    const { tabs, first, second } = twoTabs();
    const from: Source = { kind: "chat", session: 5, chat: "5" };
    const to = { kind: "pane", tab: first, pane: paneOf(tabs, first, 1), place: "left" } as const;
    expect(judgeDrop(tabs, from, to, ctx, ROOMY)).toEqual({
      ok: true,
      says: "chat 5 left of chat 1.",
    });
    expect(strip(dropOn(tabs, from, to, ctx))).toEqual(["row(5 1)", "4"]);
    expect(second in dropOn(tabs, from, to, ctx).byId).toBe(true);
  });

  it("lands another workspace's chat in a pane, and says it stays filed in its own", () => {
    const { tabs, tab } = three();
    const from: Source = { kind: "chat", session: 21, chat: "21" };
    const to = { kind: "pane", tab, pane: paneOf(tabs, tab, 2), place: "right" } as const;
    expect(judgeDrop(tabs, from, to, ctx, ROOMY)).toEqual({
      ok: true,
      says: "chat 21 right of chat 2. It stays filed in volaticloud.",
    });
    expect(shapeOf(dropOn(tabs, from, to, ctx), tab)).toBe("row(1 column(row(2 21) 3))");
  });

  it("refuses another workspace's chat where it would take the tab's first place", () => {
    // A tab's strip is still read from its first pane (#4755 D9) until it keeps a workspace
    // of its own (#4757): a drop there would carry the whole tab to the other strip.
    const { tabs, tab } = three();
    const from: Source = { kind: "chat", session: 21, chat: "21" };
    for (const place of ["left", "top", "middle"] as const) {
      const to = { kind: "pane", tab, pane: paneOf(tabs, tab, 1), place } as const;
      expect(judgeDrop(tabs, from, to, ctx, ROOMY)).toEqual({
        ok: false,
        why: "chat 21 works in volaticloud. Drop it right of or below chat 1.",
      });
      expect(dropOn(tabs, from, to, ctx)).toBe(tabs);
    }
  });
});

describe("a drop on the strip makes a tab (#4763)", () => {
  it("takes a pane out of its split into a new tab where the line is", () => {
    const { tabs, first, second } = twoTabs();
    const from = paneSource(tabs, second, 5);
    const to = { kind: "strip", at: 0 } as const;
    expect(judgeDrop(tabs, from, to, ctx)).toEqual({ ok: true, says: "A new tab with chat 5." });
    const after = dropOn(tabs, from, to, ctx);
    expect(strip(after)).toEqual(["5", "1", "4"]);
    expect(after.order.slice(1)).toEqual([first, second]);
    expect(after.byId[after.order[0]]).toMatchObject({ name: "chat 5", defaultName: "chat 5" });
    expect(after.inFront).toBe(after.order[0]);
  });

  it("opens a sidebar chat in a new tab", () => {
    const { tabs } = twoTabs();
    const after = dropOn(
      tabs,
      { kind: "chat", session: 7, chat: "7" },
      { kind: "strip", at: 2 },
      ctx,
    );
    expect(strip(after)).toEqual(["1", "row(4 5)", "7"]);
  });

  it("moves a tab along the strip", () => {
    const { tabs, first, second } = twoTabs();
    const from: Source = { kind: "tab", tab: second };
    expect(judgeDrop(tabs, from, { kind: "strip", at: 0 }, ctx)).toEqual({
      ok: true,
      says: "Move the tab here.",
    });
    const after = dropOn(tabs, from, { kind: "strip", at: 0 }, ctx);
    expect(after.order).toEqual([second, first]);
  });

  it("moves a pane alone in its tab as its tab", () => {
    const { tabs, first, second } = twoTabs();
    const after = dropOn(tabs, paneSource(tabs, first, 1), { kind: "strip", at: 2 }, ctx);
    expect(after.order).toEqual([second, first]);
  });
});

describe("a tab dragged down into a split (#4763)", () => {
  it("splits in beside a pane with all its panes, and its own place on the strip closes", () => {
    const { tabs, first, second } = twoTabs();
    const from: Source = { kind: "tab", tab: second };
    const to = { kind: "pane", tab: first, pane: paneOf(tabs, first, 1), place: "bottom" } as const;
    expect(judgeDrop(tabs, from, to, ctx, ROOMY)).toEqual({
      ok: true,
      says: "The tab chat 4 below chat 1.",
    });
    const after = dropOn(tabs, from, to, ctx);
    expect(strip(after)).toEqual(["column(1 row(4 5))"]);
  });

  it("swaps a tab of one pane in a pane's middle", () => {
    const { tabs, first, second } = twoTabs();
    const after = dropOn(
      tabs,
      { kind: "tab", tab: first },
      { kind: "pane", tab: second, pane: paneOf(tabs, second, 5), place: "middle" },
      ctx,
    );
    expect(strip(after)).toEqual(["5", "row(4 1)"]);
  });
});

describe("what a drop refuses, with its reason (#4763)", () => {
  it("refuses a split that would leave a half under 40 columns", () => {
    const { tabs, tab } = three();
    const narrow = { width: PANE_FLOOR.width * 2 - 1, height: 900 };
    const to = { kind: "pane", tab, pane: paneOf(tabs, tab, 1), place: "right" } as const;
    expect(judgeDrop(tabs, paneSource(tabs, tab, 3), to, ctx, narrow)).toEqual({
      ok: false,
      why: "Too narrow: each half would be under 40 columns.",
    });
    // The other way still fits.
    const below = { ...to, place: "bottom" } as const;
    expect(judgeDrop(tabs, paneSource(tabs, tab, 3), below, ctx, narrow).ok).toBe(true);
  });

  it("refuses a split that would leave a half under 8 rows", () => {
    const { tabs, tab } = three();
    const short = { width: 1600, height: PANE_FLOOR.height * 2 - 1 };
    const to = { kind: "pane", tab, pane: paneOf(tabs, tab, 1), place: "top" } as const;
    expect(judgeDrop(tabs, paneSource(tabs, tab, 3), to, ctx, short)).toEqual({
      ok: false,
      why: "Too short: each half would be under 8 rows.",
    });
  });

  it("takes a split exactly at the floor", () => {
    const { tabs, tab } = three();
    const exact = { width: PANE_FLOOR.width * 2, height: PANE_FLOOR.height * 2 };
    const to = { kind: "pane", tab, pane: paneOf(tabs, tab, 1), place: "left" } as const;
    expect(judgeDrop(tabs, paneSource(tabs, tab, 3), to, ctx, exact).ok).toBe(true);
  });

  it("refuses a pane dropped on itself", () => {
    const { tabs, tab } = three();
    const pane = paneOf(tabs, tab, 2);
    for (const place of ["left", "middle"] as const)
      expect(
        judgeDrop(tabs, { kind: "pane", tab, pane }, { kind: "pane", tab, pane, place }, ctx),
      ).toEqual({ ok: false, why: "That's the pane being moved." });
  });

  it("refuses a tab dropped on its own panes", () => {
    const { tabs, second } = twoTabs();
    const to = { kind: "pane", tab: second, pane: paneOf(tabs, second, 4), place: "left" } as const;
    expect(judgeDrop(tabs, { kind: "tab", tab: second }, to, ctx)).toEqual({
      ok: false,
      why: "That's the tab being moved.",
    });
  });

  it("refuses a tab of several panes in a pane's middle", () => {
    const { tabs, first, second } = twoTabs();
    const to = { kind: "pane", tab: first, pane: paneOf(tabs, first, 1), place: "middle" } as const;
    expect(judgeDrop(tabs, { kind: "tab", tab: second }, to, ctx)).toEqual({
      ok: false,
      why: "A tab of several panes goes beside a pane, not into one.",
    });
  });

  it("refuses to pick up an empty pane", () => {
    const made = three();
    const tab = made.tab;
    const two = paneOf(made.tabs, tab, 2);
    const tabs = showIn(made.tabs, tab, two, { kind: "empty", workspace: "ops" });
    const to = { kind: "pane", tab, pane: paneOf(tabs, tab, 1), place: "left" } as const;
    expect(judgeDrop(tabs, { kind: "pane", tab, pane: two }, to, ctx)).toEqual({
      ok: false,
      why: "An empty pane has nothing to move. Close it instead.",
    });
  });

  it("refuses a pane or a tab that is gone", () => {
    const { tabs, tab } = three();
    const to = { kind: "pane", tab, pane: 99, place: "left" } as const;
    expect(judgeDrop(tabs, paneSource(tabs, tab, 1), to, ctx)).toEqual({
      ok: false,
      why: "That pane has closed.",
    });
    expect(judgeDrop(tabs, { kind: "tab", tab: 99 }, { kind: "strip", at: 0 }, ctx)).toEqual({
      ok: false,
      why: "That tab has closed.",
    });
  });

  it("answers the tabs themselves for a refused drop", () => {
    const { tabs, tab } = three();
    const pane = paneOf(tabs, tab, 2);
    expect(
      dropOn(tabs, { kind: "pane", tab, pane }, { kind: "pane", tab, pane, place: "middle" }, ctx),
    ).toBe(tabs);
  });
});

describe("where in a pane the pointer is picks the place (#4763)", () => {
  it.each([
    [0.1, 0.5, "left"],
    [0.9, 0.5, "right"],
    [0.5, 0.1, "top"],
    [0.5, 0.9, "bottom"],
    [0.5, 0.5, "middle"],
    [0.26, 0.74, "middle"],
    [0.05, 0.1, "left"],
    [0.2, 0.05, "top"],
  ] as const)("at %s, %s it is %s", (x, y, place) => {
    expect(placeAt(x, y)).toBe(place);
  });
});

describe("what is drawn before a drop (#4763)", () => {
  const pane = { left: 100, top: 50, width: 800, height: 400 };
  it.each([
    ["left", { left: 100, top: 50, width: 400, height: 400 }],
    ["right", { left: 500, top: 50, width: 400, height: 400 }],
    ["top", { left: 100, top: 50, width: 800, height: 200 }],
    ["bottom", { left: 100, top: 250, width: 800, height: 200 }],
    ["middle", pane],
  ] as const)("the %s place draws its half, or the whole pane", (place, box) => {
    expect(previewOf(pane, place)).toEqual(box);
  });
});

describe("a separator keeps the floor a drop keeps (#4763)", () => {
  it("is a pane's floor for one pane, and adds up along a split's own direction", () => {
    const { tabs, tab } = three(); // row(1 column(2 3))
    const layout = tabs.byId[tab].layout;
    expect(floorAlong(layout, "row")).toBe(PANE_FLOOR.width * 2);
    expect(floorAlong(layout, "column")).toBe(PANE_FLOOR.height * 2);
    if (layout.kind !== "split") throw new Error("a split");
    expect(floorAlong(layout.children[0], "row")).toBe(PANE_FLOOR.width);
    expect(floorAlong(layout.children[1], "column")).toBe(PANE_FLOOR.height * 2);
  });
});

describe("Undo puts the last layout back (#4763)", () => {
  it("restores the tabs from before the drop", () => {
    const { tabs, first, second } = twoTabs();
    const after = dropOn(
      tabs,
      paneSource(tabs, second, 5),
      { kind: "pane", tab: first, pane: paneOf(tabs, first, 1), place: "left" },
      ctx,
    );
    const back = undone(after, tabs, after);
    expect(back && strip(back)).toEqual(["1", "row(4 5)"]);
    expect(back?.inFront).toBe(second);
  });

  it("still restores once the focus or the front tab moved", () => {
    const { tabs, first, second } = twoTabs();
    const after = dropOn(tabs, paneSource(tabs, second, 5), { kind: "strip", at: 0 }, ctx);
    const moved = selectTab(after, first);
    expect(undone(moved, tabs, after) && strip(undone(moved, tabs, after) as Tabs)).toEqual([
      "1",
      "row(4 5)",
    ]);
  });

  it("takes nothing back once the layout changed since", () => {
    const { tabs, first, second } = twoTabs();
    const after = dropOn(tabs, paneSource(tabs, second, 5), { kind: "strip", at: 0 }, ctx);
    const opened = openTab(after, 9, "9", "steward");
    expect(undone(opened, tabs, after)).toBeUndefined();
    const dragged = dropOn(after, { kind: "tab", tab: first }, { kind: "strip", at: 0 }, ctx);
    expect(undone(dragged, tabs, after)).toBeUndefined();
    const split = dropOn(
      after,
      { kind: "chat", session: 7, chat: "7" },
      { kind: "pane", tab: first, pane: paneOf(after, first, 1), place: "right" },
      ctx,
    );
    expect(undone(split, tabs, after)).toBeUndefined();
  });

  it("never hands out a number twice after an Undo", () => {
    const { tabs, first } = twoTabs();
    const after = dropOn(
      tabs,
      { kind: "chat", session: 7, chat: "7" },
      { kind: "pane", tab: first, pane: paneOf(tabs, first, 1), place: "right" },
      ctx,
    );
    expect(undone(after, tabs, after)?.named).toEqual(after.named);
  });
});
