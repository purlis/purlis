// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { layoutOf, type Reopening, type Saying, tabsOf } from "./laidOut";
import {
  back,
  closeChat,
  type Content,
  contentsOf,
  focusedChat,
  type Fronts,
  insertPane,
  noTabs,
  noteUsed,
  openTab,
  openView,
  press,
  type Pressing,
  selectTab,
  shownBeside,
  type Tabs,
  tabsIn,
  type ViewRef,
  workspaceOf,
} from "./tabs";

/**
 * **What a press does** (#4757, #5289): pressing a chat, a task or a Go shows it in a one-pane
 * tab, the replaced chat keeps running (nothing here ends it), a chat is never shown twice, a
 * view is never replaced, and each pane remembers the chat it showed before for ⌘⇧H. Where a
 * press opens is `tabs.opens.test.ts`. The window's pure layout model, beside the pane model's
 * own tests (`tabs.panes.test.ts`).
 */

const PERSONA: ViewRef = { from: null, view: "persona", key: "steward" };
const SETTINGS: ViewRef = { from: null, view: "settings", key: "project" };
const chat = (session: number): Content => ({ kind: "session", session, chat: String(session) });
const pressed = (session: number) => ({ session, chat: String(session), who: "steward" });

/** Chats 1 to 8 work in `ops`, 9 and up in `web`. */
const filedIn = (session: number) => (session < 9 ? "ops" : "web");

/** Each strip's front tab, as the window last left it. */
function pressing(fronts: Fronts = {}): Pressing {
  return { filedIn, frontOf: (workspace) => fronts[workspace] };
}

/** Every chat back under its number, as `steward <n>`. */
const allBack: Reopening = {
  chat: (session) => ({ name: String(session), who: "steward", label: null }),
  view: (view, title) => ({ view, title }),
};

/** Chat 1 in a tab, then chat 2 in a tab, which is in front: both on `ops`. */
function twoTabs(): { tabs: Tabs; first: number; second: number } {
  const tabs = openTab(openTab(noTabs(), 1, "1", "steward"), 2, "2", "steward");
  const [first, second] = tabs.order;
  return { tabs, first, second };
}

/** The one pane of tab `id`, or its first. */
function paneOf(tabs: Tabs, id: number, at = 0): number {
  return contentsOf(tabs, id)[at].pane;
}

/** What each pane of tab `id` shows, in reading order. */
function shows(tabs: Tabs, id: number): Content[] {
  return contentsOf(tabs, id).map((one) => one.content);
}

describe("a press in a tab of one pane shows the chat there (#4757, #5289)", () => {
  it("replaces what the focused pane shows and adds no tab", () => {
    const { tabs, first, second } = twoTabs();

    const { tabs: next, workspace } = press(tabs, pressed(3), pressing());

    expect(next.order).toEqual(tabs.order);
    expect(shows(next, second)).toEqual([chat(3)]);
    expect(shows(next, first)).toEqual([chat(1)]);
    expect(next.inFront).toBe(second);
    expect(focusedChat(next)).toBe(3);
    expect(workspace).toBe("ops");
  });

  it("names a tab after the chat its first pane shows now", () => {
    const { tabs, second } = twoTabs();

    const { tabs: next } = press(tabs, pressed(3), pressing());

    expect(next.byId[second].name).toBe("steward 3");
  });

  it("jumps to a chat on screen in another tab, and shows it nowhere twice", () => {
    const { tabs, first, second } = twoTabs();

    const { tabs: next } = press(tabs, pressed(1), pressing());

    expect(next.inFront).toBe(first);
    expect(focusedChat(next)).toBe(1);
    expect(shows(next, second)).toEqual([chat(2)]);
    expect(shows(next, first)).toEqual([chat(1)]);
  });

  it("jumps to a chat in another pane of the same tab, and replaces nothing", () => {
    const one = openTab(noTabs(), 1, "1", "steward");
    const split = insertPane(one, {
      tab: 1,
      pane: paneOf(one, 1),
      direction: "row",
      side: "after",
      content: chat(2),
    });

    const { tabs: next } = press(split, pressed(1), pressing());

    expect(shows(next, 1)).toEqual([chat(1), chat(2)]);
    expect(focusedChat(next)).toBe(1);
  });

  it("lands a task in the focused pane like any chat, and draws nothing over another pane", () => {
    const { tabs, first, second } = twoTabs();

    const { tabs: next } = press(tabs, pressed(5), pressing());

    expect(shows(next, second)).toEqual([chat(5)]);
    expect(next.byId[second].shows).toBeUndefined();
    expect(next.byId[first].shows).toBeUndefined();
  });

  it("fills an empty focused pane", () => {
    const { tabs, second } = twoTabs();
    const empty: Tabs = {
      ...tabs,
      byId: {
        ...tabs.byId,
        [second]: {
          ...tabs.byId[second],
          layout: {
            kind: "pane",
            pane: paneOf(tabs, second),
            content: { kind: "empty", workspace: "ops" },
          },
        },
      },
    };

    const { tabs: next } = press(empty, pressed(4), pressing());

    expect(shows(next, second)).toEqual([chat(4)]);
  });
});

describe("a pinned tab (#4757; spec story 51)", () => {
  it("keeps its pin when a press replaces what its focused pane shows", () => {
    const { tabs, second } = twoTabs();
    const pinned: Tabs = {
      ...tabs,
      byId: { ...tabs.byId, [second]: { ...tabs.byId[second], pinned: true } },
    };

    const { tabs: next } = press(pinned, pressed(3), pressing());

    expect(shows(next, second)).toEqual([chat(3)]);
    expect(next.byId[second].pinned).toBe(true);
  });
});

describe("a press never replaces a view (#4757)", () => {
  it("goes to the most recently used pane that shows a chat, and brings its tab forward", () => {
    const { tabs, first, second } = twoTabs();
    // Tab 1 used, then tab 2, then a view opened on `ops` is in front.
    const used = noteUsed(selectTab(noteUsed(selectTab(tabs, first)), second));
    const viewed = noteUsed(openView(used, PERSONA, "steward", "ops"));
    const view = viewed.inFront as number;

    const { tabs: next } = press(viewed, pressed(3), pressing());

    expect(shows(next, view)).toEqual([{ kind: "view", view: PERSONA, workspace: "ops" }]);
    expect(shows(next, second)).toEqual([chat(3)]);
    expect(shows(next, first)).toEqual([chat(1)]);
    expect(next.inFront).toBe(second);
    expect(focusedChat(next)).toBe(3);
  });

  it("takes the most recently used of them, not the first", () => {
    const { tabs, first, second } = twoTabs();
    const used = noteUsed(selectTab(noteUsed(selectTab(tabs, second)), first));
    const viewed = noteUsed(openView(used, PERSONA, "steward", "ops"));

    const { tabs: next } = press(viewed, pressed(3), pressing());

    expect(shows(next, first)).toEqual([chat(3)]);
    expect(next.inFront).toBe(first);
  });

  it("leaves a split with a view in it alone, and gets a tab of its own (#5289)", () => {
    const one = noteUsed(openTab(noTabs(), 1, "1", "steward"));
    const beside = noteUsed(
      insertPane(one, {
        tab: 1,
        pane: paneOf(one, 1),
        direction: "row",
        side: "after",
        content: { kind: "view", view: SETTINGS, workspace: "ops" },
      }),
    );

    const { tabs: next } = press(beside, pressed(3), pressing());

    expect(shows(next, 1)).toEqual([chat(1), { kind: "view", view: SETTINGS, workspace: "ops" }]);
    expect(next.order).toHaveLength(2);
    expect(focusedChat(next)).toBe(3);
  });

  it("opens a tab for the chat only where its strip has no pane but views", () => {
    const viewed = openView(noTabs(), PERSONA, "steward", "ops");

    const { tabs: next } = press(viewed, pressed(3), pressing());

    expect(next.order).toHaveLength(2);
    expect(focusedChat(next)).toBe(3);
    expect(shows(next, viewed.inFront as number)).toEqual([
      { kind: "view", view: PERSONA, workspace: "ops" },
    ]);
  });
});

describe("a chat of another workspace (#4757)", () => {
  /** `ops`'s two tabs, then `web`'s, with chat 9; `ops`'s second tab in front. */
  function twoStrips(): { tabs: Tabs; first: number; second: number; web: number } {
    const { tabs, first, second } = twoTabs();
    const withWeb = openTab(tabs, 9, "9", "steward");
    return { tabs: selectTab(withWeb, second), first, second, web: withWeb.inFront as number };
  }

  it("brings that workspace forward and lands in its front tab's focused pane", () => {
    const { tabs, first, second, web } = twoStrips();

    const { tabs: next, workspace } = press(tabs, pressed(10), pressing({ web }));

    expect(workspace).toBe("web");
    expect(next.inFront).toBe(web);
    expect(shows(next, web)).toEqual([chat(10)]);
    expect(shows(next, first)).toEqual([chat(1)]);
    expect(shows(next, second)).toEqual([chat(2)]);
  });

  it("lands in that strip's first tab when it has no front tab remembered", () => {
    const { tabs, web } = twoStrips();

    const { tabs: next } = press(tabs, pressed(10), pressing());

    expect(next.inFront).toBe(web);
    expect(focusedChat(next)).toBe(10);
  });

  it("keeps a tab on its own strip when a pane in it shows another workspace's chat", () => {
    const { tabs, second } = twoTabs();

    const { tabs: next } = press(tabs, pressed(9), pressing(), {
      tab: second,
      pane: paneOf(tabs, second),
    });

    expect(shows(next, second)).toEqual([chat(9)]);
    expect(workspaceOf(next, second, filedIn)).toBe("ops");
    expect(tabsIn(next, "ops", filedIn)).toContain(second);
    expect(tabsIn(next, "web", filedIn)).not.toContain(second);
  });

  it("tells the core the tab's own workspace, and a relaunch puts it back on that strip", () => {
    const { tabs, second } = twoTabs();
    const { tabs: next } = press(tabs, pressed(9), pressing(), {
      tab: second,
      pane: paneOf(tabs, second),
    });
    const saying: Saying = {
      workspaceOf: (id) => workspaceOf(next, id, filedIn),
      frontOf: () => undefined,
      told: () => true,
    };

    const laid = layoutOf(next, saying);
    const putBack = tabsOf(laid, noTabs(), allBack).tabs;

    expect(laid.tabs[1]).toMatchObject({ workspace: "ops", tree: { kind: "chat", chat: 9 } });
    const [, again] = putBack.order;
    expect(workspaceOf(putBack, again, filedIn)).toBe("ops");
  });
});

describe("a breadcrumb walks up the chain in its own pane (#4757)", () => {
  it("shows the chat that asked in the pane the task was in", () => {
    const one = openTab(noTabs(), 1, "1", "steward");
    const split = insertPane(one, {
      tab: 1,
      pane: paneOf(one, 1),
      direction: "row",
      side: "after",
      content: chat(5),
    });
    const task = paneOf(split, 1, 1);
    const left = selectTab(
      { ...split, byId: { ...split.byId, 1: { ...split.byId[1], focused: paneOf(split, 1) } } },
      1,
    );

    const { tabs: next } = press(left, pressed(3), pressing(), { tab: 1, pane: task });

    expect(shows(next, 1)).toEqual([chat(1), chat(3)]);
    expect(focusedChat(next)).toBe(3);
  });

  it("jumps to the chat that asked where it is on screen already", () => {
    const { tabs, first, second } = twoTabs();

    const { tabs: next } = press(tabs, pressed(1), pressing(), {
      tab: second,
      pane: paneOf(tabs, second),
    });

    expect(shows(next, second)).toEqual([chat(2)]);
    expect(next.inFront).toBe(first);
  });
});

describe("⌘⇧H goes back to the pane's previous chat (#4757)", () => {
  it("shows the chat the pane showed before, and swaps back and forth", () => {
    const { tabs, second } = twoTabs();
    const { tabs: on3 } = press(tabs, pressed(3), pressing());

    const { tabs: on2 } = back(on3, pressing(), pressed);
    const { tabs: on3again } = back(on2, pressing(), pressed);

    expect(shows(on2, second)).toEqual([chat(2)]);
    expect(shows(on3again, second)).toEqual([chat(3)]);
  });

  it("jumps where the previous chat is on screen now, and replaces nothing", () => {
    const { tabs, first, second } = twoTabs();
    const { tabs: on3 } = press(tabs, pressed(3), pressing());
    // Chat 2 is shown in the first tab now.
    const { tabs: there } = press(selectTab(on3, first), pressed(2), pressing());

    const { tabs: next } = back(selectTab(there, second), pressing(), pressed);

    expect(next.inFront).toBe(first);
    expect(focusedChat(next)).toBe(2);
    expect(shows(next, second)).toEqual([chat(3)]);
  });

  it("does nothing for a pane that showed nothing else, or whose chat has ended", () => {
    const { tabs } = twoTabs();
    const { tabs: on3 } = press(tabs, pressed(3), pressing());

    expect(back(tabs, pressing(), pressed).tabs).toBe(tabs);
    expect(
      back(on3, pressing(), (session) => (session === 2 ? undefined : pressed(session))).tabs,
    ).toBe(on3);
  });

  it("is remembered per pane: another pane's press does not move it", () => {
    const one = openTab(noTabs(), 1, "1", "steward");
    const split = insertPane(one, {
      tab: 1,
      pane: paneOf(one, 1),
      direction: "row",
      side: "after",
      content: chat(2),
    });
    const right = paneOf(split, 1, 1);
    const left = paneOf(split, 1, 0);
    const { tabs: rightOn3 } = press(split, pressed(3), pressing(), { tab: 1, pane: right });
    const { tabs: leftOn4 } = press(rightOn3, pressed(4), pressing(), { tab: 1, pane: left });
    const focusedRight = {
      ...leftOn4,
      byId: { ...leftOn4.byId, 1: { ...leftOn4.byId[1], focused: right } },
    };

    const { tabs: next } = back(focusedRight, pressing(), pressed);

    expect(shows(next, 1)).toEqual([chat(4), chat(2)]);
  });
});

describe("the sidebar's marks (#4757)", () => {
  it("are the focused pane's chat, and faintly the chats in the tab's other panes", () => {
    const one = openTab(noTabs(), 1, "1", "steward");
    const split = insertPane(one, {
      tab: 1,
      pane: paneOf(one, 1),
      direction: "row",
      side: "after",
      content: chat(2),
    });
    const withOther = selectTab(openTab(split, 3, "3", "steward"), 1);

    expect(focusedChat(withOther)).toBe(2);
    expect(shownBeside(withOther)).toEqual([1]);
  });
});

describe("a task that ends in a pane a press put it in (#4757)", () => {
  it("stays in its pane, as ended, where the window says it keeps it", () => {
    const { tabs, second } = twoTabs();
    const { tabs: on5 } = press(tabs, pressed(5), pressing());

    const kept = closeChat(on5, 5, filedIn, undefined, (session) => session === 5);

    expect(kept).toBe(on5);
    expect(shows(kept, second)).toEqual([chat(5)]);
  });

  it("takes the pane away for a chat the window does not keep", () => {
    const { tabs, second } = twoTabs();
    const { tabs: on5 } = press(tabs, pressed(5), pressing());

    const gone = closeChat(on5, 5, filedIn);

    expect(gone.order).not.toContain(second);
  });
});
