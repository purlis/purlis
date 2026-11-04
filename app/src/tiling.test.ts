// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { LaidNode } from "./bindings";
import { layoutOf, type Saying } from "./laidOut";
import {
  closeFocusedPane,
  contentsOf,
  focusPane,
  noTabs,
  openTab,
  selectTab,
  splitFocusedPane,
  type Tabs,
} from "./tabs";
import { canUndoLayout, undoLayout } from "./layoutUndo";
import { canTileAll, tileAll, zoomedLayout, zoomedOf, zoomFocusedPane } from "./tiling";

/**
 * **Tile all and Focus this pane** (#4765, spec #4750 stories 76, 77 and 85): Tile all evens a
 * tab's own panes into a grid of ⌈√n⌉ columns, filled row by row in reading order, with even
 * shares and an Undo; Focus this pane zooms the focused pane of a split for now, and the same
 * call brings the split back. The window's pure layout model, beside `tabs.panes.test.ts`.
 */

/** Every chat on `ops`, no other strip remembered, every chat known. */
const onOps: Saying = {
  workspaceOf: () => "ops",
  frontOf: () => undefined,
  told: () => true,
};

const c = (chat: number): LaidNode => ({ kind: "chat", chat });
const row = (share: number, a: LaidNode, b: LaidNode): LaidNode => ({
  kind: "split",
  direction: "row",
  share,
  a,
  b,
});
const col = (share: number, a: LaidNode, b: LaidNode): LaidNode => ({
  kind: "split",
  direction: "column",
  share,
  a,
  b,
});
const three = (x: number, y: number, z: number) => row(333, c(x), row(500, c(y), c(z)));

/**
 * One tab of `n` chats numbered 1 to n, nested unevenly as today's splits leave them: each new
 * pane halves the one before, turning between right and down. Its panes read 1 to n.
 */
function nested(n: number): Tabs {
  let tabs = openTab(noTabs(), 1, "1", "steward");
  for (let chat = 2; chat <= n; chat += 1)
    tabs = splitFocusedPane(tabs, chat % 2 === 0 ? "row" : "column", chat, String(chat));
  return tabs;
}

/** The front tab's tree as the core is told it. */
function treeOf(tabs: Tabs, at = 0): LaidNode {
  return layoutOf(tabs, onOps).tabs[at].tree;
}

/** The chats of a laid-out tree, row by row: each row of the grid a list. */
function rowsOf(node: LaidNode): number[][] {
  const across = (one: LaidNode): number[] =>
    one.kind === "chat"
      ? [one.chat]
      : one.kind === "split" && one.direction === "row"
        ? [...across(one.a), ...across(one.b)]
        : [];
  if (node.kind === "split" && node.direction === "column")
    return [...rowsOf(node.a), ...rowsOf(node.b)];
  return [across(node)];
}

/** The chats of the tab's panes, in number order, whatever the layout. */
function chatsIn(tabs: Tabs, id: number): number[] {
  return contentsOf(tabs, id)
    .map(({ content }) => (content.kind === "session" ? content.session : -1))
    .sort((a, b) => a - b);
}

describe("Tile all's grid (#4765)", () => {
  it.each([
    [2, [[1, 2]]],
    [3, [[1, 2], [3]]],
    [
      4,
      [
        [1, 2],
        [3, 4],
      ],
    ],
    [
      5,
      [
        [1, 2, 3],
        [4, 5],
      ],
    ],
    [
      6,
      [
        [1, 2, 3],
        [4, 5, 6],
      ],
    ],
    [7, [[1, 2, 3], [4, 5, 6], [7]]],
    [
      8,
      [
        [1, 2, 3],
        [4, 5, 6],
        [7, 8],
      ],
    ],
    [
      9,
      [
        [1, 2, 3],
        [4, 5, 6],
        [7, 8, 9],
      ],
    ],
  ])("tiles %i panes into ⌈√n⌉ columns, filled row by row", (n, rows) => {
    expect(rowsOf(treeOf(tileAll(nested(n), 1)))).toEqual(rows);
  });

  it("evens two panes side by side", () => {
    expect(treeOf(tileAll(nested(2), 1))).toEqual(row(500, c(1), c(2)));
  });

  it("puts the third pane alone on a second row the width of the tab", () => {
    expect(treeOf(tileAll(nested(3), 1))).toEqual(col(500, row(500, c(1), c(2)), c(3)));
  });

  it("evens five panes as three over two, each row's shares even", () => {
    expect(treeOf(tileAll(nested(5), 1))).toEqual(col(500, three(1, 2, 3), row(500, c(4), c(5))));
  });

  it("makes six panes 3 × 2 at 333 then 500", () => {
    expect(treeOf(tileAll(nested(6), 1))).toEqual(col(500, three(1, 2, 3), three(4, 5, 6)));
  });

  it("evens three rows at 333 then 500", () => {
    expect(treeOf(tileAll(nested(7), 1))).toEqual(
      col(333, three(1, 2, 3), col(500, three(4, 5, 6), c(7))),
    );
  });

  it("takes the panes in reading order: top to bottom, then left to right", () => {
    // 1 | 2 side by side, then 1 split down to put 3 under it: in the tree 1, 3, 2; on screen
    // 1 and 2 on top, 3 below.
    let tabs = nested(2);
    tabs = focusPane(tabs, 1);
    tabs = splitFocusedPane(tabs, "column", 3, "3");
    expect(treeOf(tabs)).toEqual(row(500, col(500, c(1), c(3)), c(2)));

    expect(rowsOf(treeOf(tileAll(tabs, 1)))).toEqual([[1, 2], [3]]);
  });
});

describe("Tile all keeps to the tab's own panes (#4765)", () => {
  it("never pulls in a chat nobody put there, nor drops one", () => {
    const tabs = openTab(nested(5), 9, "9", "steward");
    const [first] = tabs.order;

    const tiled = tileAll(tabs, first);

    expect(chatsIn(tiled, first)).toEqual([1, 2, 3, 4, 5]);
    expect(contentsOf(tiled, first).map((one) => one.pane)).toEqual(
      expect.arrayContaining(contentsOf(tabs, first).map((one) => one.pane)),
    );
    expect(tiled.named).toEqual(tabs.named);
  });

  it("leaves every other tab as it was", () => {
    const tabs = openTab(nested(4), 9, "9", "steward");
    const [first, second] = tabs.order;

    const tiled = tileAll(tabs, first);

    expect(tiled.byId[second]).toBe(tabs.byId[second]);
    expect(tiled.inFront).toBe(tabs.inFront);
  });

  it("tiles a tab behind the one in front, as its menu on the strip does", () => {
    const tabs = openTab(nested(3), 9, "9", "steward");
    const [first] = tabs.order;

    expect(rowsOf(treeOf(tileAll(tabs, first)))).toEqual([[1, 2], [3]]);
  });

  it("keeps the focused pane focused", () => {
    const tabs = focusPane(nested(4), 2);

    const tiled = tileAll(tabs, 1);

    expect(tiled.byId[1].focused).toBe(2);
  });

  it("is disabled at one pane, and answers the very tabs", () => {
    const tabs = nested(1);

    expect(canTileAll(tabs, 1)).toBe(false);
    expect(tileAll(tabs, 1)).toBe(tabs);
  });

  it("is enabled at two panes, and not for a tab that is not there", () => {
    expect(canTileAll(nested(2), 1)).toBe(true);
    expect(canTileAll(nested(2), 7)).toBe(false);
  });
});

describe("Undo after Tile all (#4765)", () => {
  it("puts the tab's tree and focus back as they were", () => {
    const tabs = focusPane(nested(5), 3);
    const tiled = tileAll(tabs, 1);
    expect(canUndoLayout(tiled)).toBe(true);
    expect(tiled.undo?.did).toBe("Tiled the tab's 5 panes.");

    const undone = undoLayout(tiled);

    expect(treeOf(undone)).toEqual(treeOf(tabs));
    expect(undone.byId[1].focused).toBe(3);
    expect(canUndoLayout(undone)).toBe(false);
  });

  it("is one step: a second Undo changes nothing", () => {
    const undone = undoLayout(tileAll(nested(4), 1));

    expect(undoLayout(undone)).toBe(undone);
  });

  it("is gone once the tab's layout has changed since", () => {
    const tiled = tileAll(nested(4), 1);
    const split = splitFocusedPane(tiled, "row", 9, "9");

    expect(canUndoLayout(split)).toBe(false);
    expect(undoLayout(split)).toBe(split);
  });

  it("is the window's one Undo: tiling another tab puts that one back, not both (#4763)", () => {
    let tabs = nested(3);
    tabs = splitFocusedPane(openTab(tabs, 7, "7", "steward"), "row", 8, "8");
    const [first, second] = tabs.order;
    const once = tileAll(tabs, first);

    const back = undoLayout(tileAll(once, second));

    expect(treeOf(back, 0)).toEqual(treeOf(once, 0));
    expect(treeOf(back, 1)).toEqual(treeOf(tabs, 1));
    expect(canUndoLayout(back)).toBe(false);
  });

  it("is nothing for a tab never tiled", () => {
    const tabs = nested(3);

    expect(canUndoLayout(tabs)).toBe(false);
    expect(undoLayout(tabs)).toBe(tabs);
  });
});

describe("Focus this pane in a split: zoom (#4765)", () => {
  it("zooms the focused pane, and the same call brings everything back", () => {
    const tabs = focusPane(nested(3), 2);

    const zoomed = zoomFocusedPane(tabs);
    expect(zoomedOf(zoomed, 1)).toBe(2);

    const back = zoomFocusedPane(zoomed);
    expect(zoomedOf(back, 1)).toBeUndefined();
    expect(treeOf(back)).toEqual(treeOf(tabs));
  });

  it("draws the zoomed pane alone, and the whole split once it is back", () => {
    const tabs = focusPane(nested(3), 2);
    const zoomed = zoomFocusedPane(tabs);

    expect(zoomedLayout(zoomed, 1, zoomed.byId[1].layout)).toMatchObject({
      kind: "pane",
      pane: 2,
    });
    const back = zoomFocusedPane(zoomed);
    expect(zoomedLayout(back, 1, back.byId[1].layout)).toBe(back.byId[1].layout);
  });

  it("leaves the tree alone, so the core keeps the whole split while a pane is zoomed", () => {
    const tabs = nested(3);

    expect(treeOf(zoomFocusedPane(tabs))).toEqual(treeOf(tabs));
  });

  it("does nothing in a one-pane tab, where the key is the window's Focus mode", () => {
    const tabs = nested(1);

    expect(zoomFocusedPane(tabs)).toBe(tabs);
    expect(zoomedOf(tabs, 1)).toBeUndefined();
  });

  it("is per tab: another tab in front is not zoomed, and this one still is when it comes back", () => {
    let tabs = zoomFocusedPane(nested(2));
    tabs = splitFocusedPane(openTab(tabs, 7, "7", "steward"), "row", 8, "8");
    const [first, second] = tabs.order;

    expect(zoomedOf(tabs, second)).toBeUndefined();
    expect(zoomedOf(selectTab(tabs, first), first)).toBe(2);
  });

  it("ends when the zoomed pane closes", () => {
    const zoomed = zoomFocusedPane(nested(3));

    const closed = closeFocusedPane(zoomed, () => "ops");

    expect(zoomedOf(closed, 1)).toBeUndefined();
  });

  it("ends when the tab is tiled", () => {
    const zoomed = zoomFocusedPane(nested(3));

    expect(zoomedOf(tileAll(zoomed, 1), 1)).toBeUndefined();
  });

  it("ends when the focus moves to another pane, so the pane drawn is the one with the keyboard", () => {
    const zoomed = zoomFocusedPane(focusPane(nested(3), 2));

    const moved = focusPane(zoomed, 3);

    expect(zoomedOf(moved, 1)).toBeUndefined();
    expect(zoomedLayout(moved, 1, moved.byId[1].layout)).toBe(moved.byId[1].layout);
  });

  it("ends when the zoomed pane is split, since the new pane takes the focus", () => {
    const zoomed = zoomFocusedPane(nested(2));

    expect(zoomedOf(splitFocusedPane(zoomed, "row", 9, "9"), 1)).toBeUndefined();
  });
});
