// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { OUTSIDE } from "./actions";
import { EVEN, layoutOf, pathTo, type Reopening, type Saying, tabsOf, withShare } from "./laidOut";
import {
  noTabs,
  openTab,
  openView,
  openViewBeside,
  openViewBesidePane,
  pinsOf,
  selectTab,
  setPinned,
  showIn,
  splitFocusedPane,
  type Tabs,
  type ViewRef,
} from "./tabs";

const PERSONA: ViewRef = { from: null, view: "persona", key: "steward" };

/** Every chat on `ops`, no other strip remembered, every chat known. */
const onOps: Saying = {
  workspaceOf: () => "ops",
  frontOf: () => undefined,
  told: () => true,
};

/** Every chat came back under its number, as `steward <n>`. */
const allBack: Reopening = {
  chat: (session) => ({ name: String(session), who: "steward", label: null }),
  view: (view, title) => ({ view, title }),
};

/** `tabs` with its first tab's only pane emptied, as the empty pane's own close leaves it. */
function withEmptyPane(tabs: Tabs): Tabs {
  return showIn(tabs, tabs.order[0], 1, { kind: "empty", workspace: "ops" });
}

/** Chats 3 and 4 split side by side in one tab, the first holding a third. */
function twoSplit(): Tabs {
  const split = splitFocusedPane(openTab(noTabs(), 3, "3", "steward"), "row", 4, "4");
  const id = split.order[0];
  return withShare(split, id, "", 333);
}

describe("the window's tabs as the core holds them (#4751)", () => {
  it("says a split, its direction, its share and its focused pane's path", () => {
    const laid = layoutOf(twoSplit(), onOps);

    expect(laid).toEqual({
      strip: "ops",
      tabs: [
        {
          workspace: "ops",
          pinned: false,
          window: 0,
          front: true,
          focused: "b",
          tree: {
            kind: "split",
            direction: "row",
            share: 333,
            a: { kind: "chat", chat: 3 },
            b: { kind: "chat", chat: 4 },
          },
        },
      ],
    });
  });

  it("two chats split together, with their sizes, come back the same", () => {
    const before = twoSplit();

    const back = tabsOf(layoutOf(before, onOps), noTabs(), allBack);

    expect(back.placed).toEqual(new Set([3, 4]));
    expect(back.inFront).toBe(back.tabs.order[0]);
    expect(layoutOf(selectTab(back.tabs, back.inFront ?? 0), onOps)).toEqual(
      layoutOf(before, onOps),
    );
    expect(back.tabs.byId[back.tabs.order[0]].name).toBe("steward 3");
  });

  it("a view beside a chat comes back, waiting for a press", () => {
    const before = openViewBeside(openTab(noTabs(), 3, "3", "steward"), 3, PERSONA, "ops");

    const back = tabsOf(layoutOf(before, onOps), noTabs(), allBack);

    const layout = back.tabs.byId[back.tabs.order[0]].layout;
    expect(layout.kind === "split" && layout.children[1]).toMatchObject({
      kind: "pane",
      content: { kind: "view", view: PERSONA, workspace: "ops", waits: true },
    });
  });

  it("a view beside a chat comes back under its own title, not its view's word (#5577)", () => {
    const file: ViewRef = { from: null, view: "file", key: "/repo/notes.md" };
    const before = openViewBesidePane(
      openTab(noTabs(), 3, "3", "steward"),
      1,
      file,
      "notes.md",
      "ops",
      "row",
    );

    const laid = layoutOf(before, onOps);
    const tree = laid.tabs[0].tree;
    expect(tree.kind === "split" && tree.b.kind === "view" && tree.b.view.title).toBe("notes.md");
    const back = tabsOf(laid, noTabs(), allBack);
    const layout = back.tabs.byId[back.tabs.order[0]].layout;
    expect(layout.kind === "split" && layout.children[1]).toMatchObject({
      content: { kind: "view", view: file, title: "notes.md" },
    });
  });

  it("a tab that opens on a view comes back under its title and its pin", () => {
    const before = setPinned(openView(noTabs(), PERSONA, "steward", "ops"), 1, true);

    const back = tabsOf(layoutOf(before, onOps), noTabs(), allBack);

    const tab = back.tabs.byId[back.tabs.order[0]];
    expect(tab.name).toBe("steward");
    expect(tab.pinned).toBe(true);
  });

  it("a pinned chat tab comes back pinned by the layout, not by its chat (#4762)", () => {
    // The pin is the tab's: the record's `tabs[].pinned` is what says it, and the chat's own
    // field has retired.
    const before = openTab(openTab(noTabs(), 1), 2);
    const laid = layoutOf(setPinned(before, before.order[1], true), onOps);

    const back = tabsOf(laid, noTabs(), allBack);

    expect(back.tabs.order.map((id) => back.tabs.byId[id].pinned === true)).toEqual([false, true]);
  });

  it("a tab whose only pane is empty can be pinned, and the layout says its pin (#4935)", () => {
    const before = withEmptyPane(openTab(noTabs(), 1));
    const id = before.order[0];

    const pinned = setPinned(before, id, true);

    expect(pinsOf(pinned)(id)).toBe(true);
    expect(layoutOf(pinned, onOps).tabs[0]).toMatchObject({
      pinned: true,
      tree: { kind: "empty" },
    });
    const back = tabsOf(layoutOf(pinned, onOps), noTabs(), allBack);
    expect(back.tabs.byId[back.tabs.order[0]].pinned).toBe(true);
  });

  it("a tab's pin stays when its first pane shows another chat (#4935)", () => {
    const before = setPinned(openTab(openTab(noTabs(), 1), 2), 1, true);
    const [first, second] = before.order;

    // Its first pane is emptied, then shows chat 3: the tab is the same tab, and pinned.
    const emptied = showIn(before, first, 1, { kind: "empty", workspace: "ops" });
    const other = showIn(emptied, first, 1, { kind: "session", session: 3, chat: "3" });

    expect(pinsOf(emptied)(first)).toBe(true);
    expect(pinsOf(other)(first)).toBe(true);
    expect(pinsOf(other)(second)).toBe(false);
    expect(layoutOf(other, onOps).tabs.map((tab) => tab.pinned)).toEqual([true, false]);
  });

  it("a tab unpinned says so, and leaves no pin behind (#4935)", () => {
    const pinned = setPinned(openTab(noTabs(), 1), 1, true);

    const unpinned = setPinned(pinned, 1, false);

    expect(pinsOf(unpinned)(1)).toBe(false);
    expect(unpinned.byId[1]).not.toHaveProperty("pinned");
    expect(setPinned(unpinned, 1, false)).toBe(unpinned);
  });

  it("each strip records its front tab, and the strip outside every workspace is null", () => {
    let tabs = openTab(noTabs(), 1, "1");
    tabs = openTab(tabs, 2, "2");
    tabs = openTab(tabs, 3, "3");
    const [one, two, three] = tabs.order;
    const say: Saying = {
      ...onOps,
      workspaceOf: (id) => (id === one ? OUTSIDE : "ops"),
      frontOf: (workspace) => (workspace === OUTSIDE ? one : undefined),
    };

    const laid = layoutOf(selectTab(tabs, two), say);

    expect(laid.strip).toBe("ops");
    expect(laid.tabs.map((tab) => [tab.workspace, tab.front])).toEqual([
      [null, true],
      ["ops", true],
      ["ops", false],
    ]);
    expect(three).toBeDefined();
    const back = tabsOf(laid, noTabs(), allBack);
    expect(back.fronts[OUTSIDE]).toBe(back.tabs.order[0]);
    expect(back.inFront).toBe(back.tabs.order[1]);
  });

  it("a chat the core does not have open is an empty pane, and an empty pane comes back empty (#4755)", () => {
    const laid = layoutOf(twoSplit(), { ...onOps, told: (session) => session !== 4 });
    expect(laid.tabs[0].tree).toMatchObject({ b: { kind: "empty" } });

    const back = tabsOf(laid, noTabs(), allBack);

    const layout = back.tabs.byId[back.tabs.order[0]].layout;
    expect(layout).toMatchObject({
      kind: "split",
      share: 333,
      children: [
        { kind: "pane", content: { session: 3 } },
        { kind: "pane", content: { kind: "empty", workspace: "ops" } },
      ],
    });
  });

  it("a chat that did not come back leaves its split to the other side, and a tab of nothing is not put back", () => {
    const laid = layoutOf(openTab(twoSplit(), 9, "9"), onOps);

    const back = tabsOf(laid, noTabs(), {
      ...allBack,
      chat: (session) => (session === 3 ? allBack.chat(session) : undefined),
    });

    expect(back.tabs.order).toHaveLength(1);
    expect(back.placed).toEqual(new Set([3]));
    expect(back.tabs.byId[back.tabs.order[0]].focused).toBe(
      (back.tabs.byId[back.tabs.order[0]].layout as { pane: number }).pane,
    );
  });

  it("a chat named in an earlier pane is not put back twice", () => {
    const laid = layoutOf(openTab(twoSplit(), 3, "3"), onOps);

    const back = tabsOf(laid, noTabs(), allBack);

    expect(back.tabs.order).toHaveLength(1);
  });

  it("a share is set at the split's path, and setting it again changes nothing", () => {
    const tabs = twoSplit();
    const id = tabs.order[0];

    expect(withShare(tabs, id, "", 333)).toBe(tabs);
    expect(withShare(tabs, id, "a", 100)).toBe(tabs);
    const moved = withShare(tabs, id, "", 750);
    expect(layoutOf(moved, onOps).tabs[0].tree).toMatchObject({ share: 750 });
  });

  it("a split never moved is even", () => {
    const split = splitFocusedPane(openTab(noTabs(), 3, "3"), "column", 4, "4");

    expect(layoutOf(split, onOps).tabs[0].tree).toMatchObject({
      direction: "column",
      share: EVEN,
    });
  });

  it("a pane's path is the sides taken from the root", () => {
    const tabs = twoSplit();
    const tab = tabs.byId[tabs.order[0]];
    const [a, b] = tab.layout.kind === "split" ? tab.layout.children : [];

    expect(pathTo(tab.layout, (a as { pane: number }).pane)).toBe("a");
    expect(pathTo(tab.layout, (b as { pane: number }).pane)).toBe("b");
    expect(pathTo(tab.layout, 999)).toBeUndefined();
  });
});
