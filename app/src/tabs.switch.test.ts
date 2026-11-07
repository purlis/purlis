// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  EMPTY_TAB,
  chatNamed,
  closeLayout,
  dropEmptyTabsOfGoneWorkspaces,
  newChatNumber,
  noTabs,
  openEmptyTab,
  openTab,
  openNew,
  press,
  workspaceOf,
  type Tabs,
} from "./tabs";

/**
 * **The switch: a chat isn't a tab** (#4758). The strip's `+` opens a tab with one empty pane
 * (spec story 13). Where a chat the person starts goes is `tabs.opens.test.ts`.
 */

const filedIn = (on: Record<number, string>) => (session: number) => on[session] ?? "ops";
const at = (on: Record<number, string> = {}) => ({
  filedIn: filedIn(on),
  frontOf: () => undefined,
});

describe("the strip's + (#4758; spec story 13)", () => {
  it("opens a tab holding one empty pane, in front, after the strip's last tab", () => {
    const before = openTab(noTabs(), 1, "1", "steward");

    const tabs = openEmptyTab(before, "ops");

    const id = tabs.inFront ?? -1;
    expect(tabs.order).toEqual([...before.order, id]);
    const tab = tabs.byId[id];
    expect(tab.name).toBe(EMPTY_TAB);
    expect(tab.layout).toEqual({
      kind: "pane",
      pane: tab.focused,
      content: { kind: "empty", workspace: "ops" },
    });
    expect(workspaceOf(tabs, id, filedIn({}))).toBe("ops");
  });

  it("names it Empty tab until it holds something (#5292)", () => {
    const tabs = openEmptyTab(noTabs(), "ops");
    const id = tabs.inFront ?? -1;
    expect(tabs.byId[id].name).toBe("Empty tab");

    const landed = press(tabs, { session: 2, chat: "2", who: "steward" }, at());

    expect(landed.tabs.byId[id].name).toBe("steward 2");
  });

  it("is a tab New chat then fills, adding no other", () => {
    const tabs: Tabs = openEmptyTab(openTab(noTabs(), 1, "1", "steward"), "ops");

    const landed = press(tabs, { session: 2, chat: "2", who: "steward" }, at());

    expect(landed.tabs.order).toEqual(tabs.order);
    const front = landed.tabs.byId[landed.tabs.inFront ?? -1];
    expect(front.layout).toMatchObject({ kind: "pane", content: { session: 2 } });
  });
});

describe("a new chat's number (#4758)", () => {
  it("is 1 for a fresh window's first chat, though its empty tab is tab 1", () => {
    const tabs = openEmptyTab(noTabs(), "ops");

    expect(newChatNumber(tabs)).toBe(1);
  });

  it("counts every chat the window named, so the next is 2 though it takes the same pane", () => {
    const tabs = chatNamed(openEmptyTab(noTabs(), "ops"), 1);

    expect(newChatNumber(tabs)).toBe(2);
  });

  it("is never one a chat still running has, after a close empties that chat's tab", () => {
    const first = press(
      chatNamed(openEmptyTab(noTabs(), "ops"), 1),
      { session: 1, chat: "1", who: "steward" },
      at(),
    );
    const emptied = closeLayout(first.tabs, first.tabs.inFront ?? -1, filedIn({}));

    expect(newChatNumber(emptied)).toBe(2);
  });
});

describe("an empty tab of a workspace the plane no longer has (#4758)", () => {
  it("goes, and the tabs of the workspaces still there stay", () => {
    const tabs = openEmptyTab(openEmptyTab(noTabs(), "alpha"), "beta");

    const left = dropEmptyTabsOfGoneWorkspaces(tabs, (workspace) => workspace === "beta");

    expect(left.order).toEqual([2]);
    expect(left.inFront).toBe(2);
  });

  it("is the same tabs where every workspace is still there", () => {
    const tabs = openEmptyTab(noTabs(), "alpha");

    expect(dropEmptyTabsOfGoneWorkspaces(tabs, () => true)).toBe(tabs);
  });
});

describe("a chat the person starts in a + tab (#4758, #5289)", () => {
  it("lands in its empty pane, though the chat is filed on another strip", () => {
    const before = openTab(noTabs(), 1, "1", "steward");
    const tabs = openEmptyTab(before, "ops");
    const plus = tabs.inFront ?? -1;

    const landed = openNew(
      tabs,
      { session: 2, chat: "2", who: "steward" },
      { filedIn: () => "alpha", frontOf: () => before.inFront },
    );

    expect(landed.tabs.inFront).toBe(plus);
    expect(landed.tabs.byId[plus].layout).toMatchObject({ content: { session: 2 } });
    expect(landed.tabs.byId[before.inFront ?? -1].layout).toMatchObject({
      content: { session: 1 },
    });
  });
});
