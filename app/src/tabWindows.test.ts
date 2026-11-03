// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { LaidNode, LaidOut, LaidTab } from "./bindings";
import { layoutOf, type Reopening, type Saying, tabsOf } from "./laidOut";
import { movedBetweenWindows, tornOff } from "./tabWindows";
import {
  noTabs,
  openTab,
  openView,
  selectTab,
  splitFocusedPane,
  type Tabs,
  type ViewRef,
} from "./tabs";

const onOps: Saying = {
  workspaceOf: () => "ops",
  frontOf: () => undefined,
  told: () => true,
};

const allBack: Reopening = {
  chat: (session) => ({ name: String(session), who: "steward", label: null }),
  view: (view, title) => ({ view, title }),
};

const PERSONA: ViewRef = { from: null, view: "persona", key: "steward" };

const chat = (number: number): LaidNode => ({ kind: "chat", chat: number });

function tab(window: number, front: boolean, tree: LaidNode): LaidTab {
  return { workspace: "ops", pinned: false, window, front, focused: "", tree };
}

/** The chats each tab of `tabs` shows, in strip order. */
function shown(tabs: Tabs): number[][] {
  return tabs.order.map((id) => {
    const layout = tabs.byId[id].layout;
    const each = (node: typeof layout): number[] =>
      node.kind === "split"
        ? [...each(node.children[0]), ...each(node.children[1])]
        : node.content.kind === "session"
          ? [node.content.session]
          : [];
    return each(layout);
  });
}

function inFrontOf(tabs: Tabs): number[] {
  return tabs.inFront === undefined ? [] : shown({ ...tabs, order: [tabs.inFront] })[0];
}

describe("a tab window's share of the project's tabs (#4768)", () => {
  it("a tab window says every one of its tabs as its own, the one in front its front", () => {
    let tabs = openTab(noTabs(), 5, "5");
    tabs = openTab(tabs, 6, "6");
    tabs = selectTab(tabs, tabs.order[0]);

    const laid = layoutOf(tabs, onOps, 2);

    expect(laid.tabs.map((one) => [one.window, one.front])).toEqual([
      [2, true],
      [2, false],
    ]);
  });

  it("the project window draws only its own tabs, and gives no tab of its own to a chat a tab window shows", () => {
    const laid: LaidOut = {
      strip: "ops",
      tabs: [
        tab(0, true, chat(1)),
        tab(1, true, chat(2)),
        tab(0, false, chat(3)),
        tab(1, false, { kind: "empty" }),
      ],
    };

    const back = tabsOf(laid, noTabs(), allBack);

    expect(shown(back.tabs)).toEqual([[1], [3]]);
    expect(back.placed.has(2)).toBe(true);
    expect(inFrontOf({ ...back.tabs, inFront: back.inFront })).toEqual([1]);
  });

  it("a tab window draws only its own tabs, its first front tab in front", () => {
    const laid: LaidOut = {
      strip: "ops",
      tabs: [
        tab(0, true, chat(1)),
        tab(1, false, chat(2)),
        { ...tab(1, true, chat(4)), workspace: null },
        tab(2, true, chat(3)),
      ],
    };

    const back = tabsOf(laid, noTabs(), allBack, 1);

    expect(shown(back.tabs)).toEqual([[2], [4]]);
    expect(inFrontOf({ ...back.tabs, inFront: back.inFront })).toEqual([4]);
    expect(back.placed).toEqual(new Set([1, 2, 3, 4]));
  });
});

describe("what a window does when the core moves tabs between windows (#4768)", () => {
  it("a tab moved to a new window leaves the project window, and the tab beside it comes forward", () => {
    let tabs = openTab(noTabs(), 1, "1");
    tabs = openTab(tabs, 2, "2");
    const laid: LaidOut = {
      strip: "ops",
      tabs: [tab(0, true, chat(1)), tab(1, true, chat(2))],
    };

    const moved = movedBetweenWindows(laid, tabs, 0, allBack, () => "ops");

    expect(moved.changed).toBe(true);
    expect(shown(moved.tabs)).toEqual([[1]]);
    expect(inFrontOf(moved.tabs)).toEqual([1]);
  });

  it("a pane moved to another window leaves its split to the pane beside it, so its chat is drawn once", () => {
    let tabs = openTab(noTabs(), 3, "3");
    tabs = openTab(tabs, 1, "1");
    tabs = splitFocusedPane(tabs, "row", 2);
    const laid: LaidOut = {
      strip: "ops",
      tabs: [tab(0, false, chat(3)), tab(0, true, chat(1)), tab(1, true, chat(2))],
    };

    const moved = movedBetweenWindows(laid, tabs, 0, allBack, () => "ops");

    expect(moved.changed).toBe(true);
    expect(shown(moved.tabs)).toEqual([[3], [1]]);
    expect(inFrontOf(moved.tabs)).toEqual([1]);
    const front = moved.tabs.byId[moved.tabs.inFront as number];
    expect(front.layout).toMatchObject({ kind: "pane", pane: front.focused });
  });

  it("a tab moved into a window that is open lands as its front tab", () => {
    const tabs = openTab(noTabs(), 5, "5");
    const laid: LaidOut = {
      strip: "ops",
      tabs: [tab(0, true, chat(1)), tab(2, false, chat(5)), tab(2, true, chat(2))],
    };

    const moved = movedBetweenWindows(laid, tabs, 2, allBack, () => "ops");

    expect(shown(moved.tabs)).toEqual([[5], [2]]);
    expect(inFrontOf(moved.tabs)).toEqual([2]);
  });

  it("tabs sent back from a closed tab window go behind the front tab and nothing they hold comes forward", () => {
    let tabs = openTab(noTabs(), 1, "1");
    tabs = openTab(tabs, 3, "3");
    tabs = selectTab(tabs, tabs.order[0]);
    const laid: LaidOut = {
      strip: "ops",
      tabs: [tab(0, true, chat(1)), tab(0, false, chat(5)), tab(0, false, chat(3))],
    };

    const moved = movedBetweenWindows(laid, tabs, 0, allBack, () => "ops");

    expect(shown(moved.tabs)).toEqual([[1], [5], [3]]);
    expect(inFrontOf(moved.tabs)).toEqual([1]);
  });

  it("a view tab moves between windows as a chat's does", () => {
    let tabs = openTab(noTabs(), 1, "1");
    tabs = openView(tabs, PERSONA, "steward", "ops");
    const laid: LaidOut = {
      strip: "ops",
      tabs: [
        tab(0, true, chat(1)),
        tab(1, true, {
          kind: "view",
          view: {
            ...PERSONA,
            title: "steward",
            workspace: "ops",
            at: 0,
            active: false,
            pinned: false,
            split: null,
          },
        }),
      ],
    };

    const moved = movedBetweenWindows(laid, tabs, 0, allBack, () => "ops");

    expect(moved.tabs.order).toHaveLength(1);
    expect(shown(moved.tabs)).toEqual([[1]]);
  });

  it("nothing moved changes nothing", () => {
    const tabs = openTab(noTabs(), 1, "1");
    const laid: LaidOut = { strip: "ops", tabs: [tab(0, true, chat(1)), tab(1, true, chat(2))] };

    const moved = movedBetweenWindows(laid, tabs, 0, allBack, () => "ops");

    expect(moved.changed).toBe(false);
    expect(moved.tabs).toBe(tabs);
  });
});

describe("a tab dragged off the window (#4768)", () => {
  const window = { width: 1200, height: 800 };

  it("is torn off once the pointer is let go past any edge of the window", () => {
    expect(tornOff({ x: -5, y: 300 }, window)).toBe(true);
    expect(tornOff({ x: 1210, y: 300 }, window)).toBe(true);
    expect(tornOff({ x: 600, y: -2 }, window)).toBe(true);
    expect(tornOff({ x: 600, y: 805 }, window)).toBe(true);
  });

  it("stays a move along the strip while the pointer is inside the window, its edge included", () => {
    expect(tornOff({ x: 0, y: 0 }, window)).toBe(false);
    expect(tornOff({ x: 1200, y: 800 }, window)).toBe(false);
    expect(tornOff({ x: 600, y: 400 }, window)).toBe(false);
  });

  it("is not torn off where the pointer was never seen", () => {
    expect(tornOff(undefined, window)).toBe(false);
  });
});
