// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { layoutOf, type Reopening, type Saying, tabsOf, withShare } from "./laidOut";
import {
  type Content,
  contentsOf,
  focusedChat,
  insertPane,
  noTabs,
  openTab,
  selectTab,
  showIn,
  splitFocusedPane,
  type Tabs,
  type ViewRef,
} from "./tabs";

/**
 * **A pane shows a chat, a view, or nothing** (#4755): any open chat, not only its tab's own or
 * a task of it; a split can be inserted on either side of a pane; and each tab keeps its own
 * sizes. The window's pure layout model, beside the tab model's own tests (`tabs.test.ts`) and
 * the round trip through the core (`laidOut.test.ts`).
 */

const PERSONA: ViewRef = { from: null, view: "persona", key: "steward" };
const EMPTY: Content = { kind: "empty", workspace: "ops" };
const chat = (session: number): Content => ({ kind: "session", session, chat: String(session) });

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

/** Chat 1 in a tab of its own, then chat 2 in a tab of its own, which is in front. */
function twoTabs(): { tabs: Tabs; first: number; second: number } {
  const tabs = openTab(openTab(noTabs(), 1, "1", "steward"), 2, "2", "steward");
  const [first, second] = tabs.order;
  return { tabs, first, second };
}

/** The one pane of tab `id`. */
function onlyPane(tabs: Tabs, id: number): number {
  const [one] = contentsOf(tabs, id);
  return one.pane;
}

describe("a pane shows any chat (#4755)", () => {
  it("shows a chat that is neither its tab's own nor a task of it", () => {
    const { tabs, first } = twoTabs();
    const shown = selectTab(showIn(tabs, first, onlyPane(tabs, first), chat(7)), first);

    expect(contentsOf(shown, first).map((one) => one.content)).toEqual([chat(7)]);
    expect(focusedChat(shown)).toBe(7);
    expect(layoutOf(shown, onOps).tabs[0].tree).toEqual({ kind: "chat", chat: 7 });
  });

  it("refuses a chat another pane already shows, so no chat is on screen twice", () => {
    const { tabs, first } = twoTabs();

    expect(showIn(tabs, first, onlyPane(tabs, first), chat(2))).toBe(tabs);
  });

  it("answers the very tabs when the pane already shows it", () => {
    const { tabs, first } = twoTabs();

    expect(showIn(tabs, first, onlyPane(tabs, first), chat(1))).toBe(tabs);
  });

  it("shows a chat whose own pane has a task drawn over it, since that chat is on no screen", () => {
    const { tabs, first, second } = twoTabs();
    const pane = onlyPane(tabs, second);
    const switched: Tabs = {
      ...tabs,
      byId: { ...tabs.byId, [second]: { ...tabs.byId[second], shows: { [pane]: 5 } } },
    };

    const shown = showIn(switched, first, onlyPane(tabs, first), chat(2));

    expect(contentsOf(shown, first).map((one) => one.content)).toEqual([chat(2)]);
    expect(showIn(switched, first, onlyPane(tabs, first), chat(5))).toBe(switched);
  });

  it("answers the very tabs for a pane or a tab that is not there", () => {
    const { tabs, first } = twoTabs();

    expect(showIn(tabs, first, 99, chat(7))).toBe(tabs);
    expect(showIn(tabs, 99, 1, chat(7))).toBe(tabs);
  });

  it("forgets a task the pane showed in place of its chat, and shows what it is given", () => {
    const { tabs, second } = twoTabs();
    const pane = onlyPane(tabs, second);
    const switched: Tabs = {
      ...tabs,
      byId: { ...tabs.byId, [second]: { ...tabs.byId[second], shows: { [pane]: 5 } } },
    };

    const shown = showIn(switched, second, pane, chat(7));

    expect(shown.byId[second].shows).toBeUndefined();
    expect(focusedChat(shown)).toBe(7);
  });
});

describe("a pane shows nothing (#4755)", () => {
  it("is an empty pane, which the core is told as empty and on screen as no chat", () => {
    const { tabs, second } = twoTabs();

    const cleared = showIn(tabs, second, onlyPane(tabs, second), EMPTY);

    expect(contentsOf(cleared, second).map((one) => one.content)).toEqual([EMPTY]);
    expect(focusedChat(cleared)).toBeUndefined();
    expect(layoutOf(cleared, onOps).tabs[1].tree).toEqual({ kind: "empty" });
  });

  it("comes back after a relaunch in its place in a split", () => {
    const split = insertPane(openTab(noTabs(), 3, "3", "steward"), {
      tab: 1,
      pane: 1,
      direction: "row",
      side: "after",
      content: EMPTY,
    });

    const back = tabsOf(layoutOf(split, onOps), noTabs(), allBack);

    expect(layoutOf(back.tabs, onOps).tabs[0].tree).toEqual({
      kind: "split",
      direction: "row",
      share: 500,
      a: { kind: "chat", chat: 3 },
      b: { kind: "empty" },
    });
  });

  it("comes back as a tab of its own when it is all the tab holds, called New chat", () => {
    const { tabs, second } = twoTabs();
    const cleared = showIn(tabs, second, onlyPane(tabs, second), EMPTY);

    const back = tabsOf(layoutOf(cleared, onOps), noTabs(), allBack);

    expect(back.tabs.order).toHaveLength(2);
    const last = back.tabs.byId[back.tabs.order[1]];
    expect(last.layout).toMatchObject({ kind: "pane", content: EMPTY });
    expect(last.name).toBe("New chat");
  });
});

describe("inserting a pane on either side (#4755)", () => {
  const one = () => openTab(noTabs(), 3, "3", "steward");

  it("puts a pane on the left as the split's first child, at an even share", () => {
    const tabs = insertPane(one(), {
      tab: 1,
      pane: 1,
      direction: "row",
      side: "before",
      content: chat(4),
    });

    expect(layoutOf(tabs, onOps).tabs[0]).toMatchObject({
      focused: "a",
      tree: {
        kind: "split",
        direction: "row",
        share: 500,
        a: { kind: "chat", chat: 4 },
        b: { kind: "chat", chat: 3 },
      },
    });
  });

  it("puts a pane on top as a column split's first child", () => {
    const tabs = insertPane(one(), {
      tab: 1,
      pane: 1,
      direction: "column",
      side: "before",
      content: { kind: "view", view: PERSONA, workspace: "ops" },
    });

    const tree = layoutOf(tabs, onOps).tabs[0].tree;
    expect(tree).toMatchObject({ kind: "split", direction: "column" });
    expect(tree.kind === "split" && tree.a.kind).toBe("view");
    expect(tree.kind === "split" && tree.b).toEqual({ kind: "chat", chat: 3 });
  });

  it("puts a pane on the right or below as the second child, as Split right always has", () => {
    const after = insertPane(one(), {
      tab: 1,
      pane: 1,
      direction: "column",
      side: "after",
      content: chat(4),
    });

    expect(layoutOf(after, onOps).tabs[0]).toMatchObject({
      focused: "b",
      tree: {
        kind: "split",
        direction: "column",
        share: 500,
        a: { kind: "chat", chat: 3 },
        b: { kind: "chat", chat: 4 },
      },
    });
  });

  it("inserts beside a pane that is not focused, deep in a split, in a tab behind", () => {
    let tabs = splitFocusedPane(one(), "row", 4, "4");
    tabs = openTab(tabs, 5, "5", "steward");
    const [behind] = tabs.order;

    const inserted = insertPane(tabs, {
      tab: behind,
      pane: 1,
      direction: "column",
      side: "before",
      content: EMPTY,
    });

    expect(inserted.inFront).toBe(tabs.inFront);
    expect(layoutOf(inserted, onOps).tabs[0].tree).toEqual({
      kind: "split",
      direction: "row",
      share: 500,
      a: {
        kind: "split",
        direction: "column",
        share: 500,
        a: { kind: "empty" },
        b: { kind: "chat", chat: 3 },
      },
      b: { kind: "chat", chat: 4 },
    });
  });

  it("inserts nothing for a chat another pane already shows", () => {
    const tabs = splitFocusedPane(one(), "row", 4, "4");

    expect(
      insertPane(tabs, { tab: 1, pane: 1, direction: "row", side: "before", content: chat(4) }),
    ).toBe(tabs);
  });

  it("inserts nothing beside a pane that is not there", () => {
    const tabs = one();

    expect(
      insertPane(tabs, { tab: 1, pane: 9, direction: "row", side: "before", content: EMPTY }),
    ).toBe(tabs);
  });
});

describe("sizes belong to each tab (#4755)", () => {
  it("a split moved in one tab leaves another tab of the same shape as it was", () => {
    let tabs = splitFocusedPane(openTab(noTabs(), 1, "1"), "row", 2, "2");
    tabs = splitFocusedPane(openTab(tabs, 3, "3"), "row", 4, "4");
    const [first, second] = tabs.order;

    const moved = withShare(tabs, first, "", 250);

    const shares = layoutOf(moved, onOps).tabs.map((tab) =>
      tab.tree.kind === "split" ? tab.tree.share : undefined,
    );
    expect(shares).toEqual([250, 500]);
    expect(moved.byId[second]).toBe(tabs.byId[second]);
  });
});
