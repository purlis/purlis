// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { firstPaneNameOf, type DropContext } from "./paneDrop";
import {
  closePane,
  namedForFirstPanes,
  noTabs,
  openTab,
  openView,
  panes,
  splitFocusedPane,
  tabNameOf,
  type Content,
  type Tabs,
} from "./tabs";
import { chatRenamed, nameTab, renameOf } from "./tabRenaming";

/**
 * **What a tab's Rename renames** (#5343, spec #5339): a tab with one chat renames the chat,
 * unless the person names only the tab; a split names only the tab; a view's tab has none. A
 * chat's rename names every tab it leads that has no name of its own.
 */

const names: Record<number, string> = { 1: "steward 1", 2: "devops 2" };
const nameOf = (content: Content) =>
  content.kind === "session" ? (names[content.session] ?? String(content.session)) : "New chat";
const ctx: DropContext = { filedIn: () => "ops", name: nameOf };
const named = (before: Tabs, after: Tabs) =>
  namedForFirstPanes(before, after, (content, was) => firstPaneNameOf(content, was, ctx));

const ID = 1;
const one = () => openTab(noTabs(), 1, "1", "steward");
const split = () => named(one(), splitFocusedPane(one(), "row", 2, "2"));
const second = (tabs: Tabs) => {
  const found = panes(tabs.byId[ID].layout).find(
    (pane) => pane.content.kind === "session" && pane.content.session === 2,
  );
  if (found === undefined) throw new Error("no pane shows chat 2");
  return found.pane;
};

describe("what a tab's Rename renames", () => {
  it("renames the chat of a tab with one chat", () => {
    expect(renameOf(one(), ID)).toEqual({ kind: "chat", session: 1 });
  });

  it("names only a split tab, and says how many panes it holds", () => {
    expect(renameOf(split(), ID)).toEqual({ kind: "tab", panes: 2 });
  });

  it("has nothing to rename on a view's tab", () => {
    const view = openView(noTabs(), { from: null, view: "activity", key: "" }, "Activity", "ops");
    expect(renameOf(view, view.order[0])).toBeUndefined();
  });

  it("renames the chat again once a split closes back to one pane", () => {
    const before = split();
    const closed = named(
      before,
      closePane(before, ID, second(before), () => "ops"),
    );
    expect(renameOf(closed, ID)).toEqual({ kind: "chat", session: 1 });
  });
});

describe("a tab's own name", () => {
  it("is what the tab is called, and the chat's name is left as it was", () => {
    const tabs = nameTab(one(), ID, "the build");
    expect(tabNameOf(tabs.byId[ID])).toBe("the build");
    expect(tabs.byId[ID].name).toBe("steward 1");
  });

  it("is cleared, and the tab goes back to its first chat's name", () => {
    const tabs = nameTab(nameTab(split(), ID, "the build"), ID, undefined);
    expect(tabs.byId[ID].own).toBeUndefined();
    expect(tabNameOf(tabs.byId[ID])).toBe("steward 1");
  });

  it("is kept through a close back to one pane", () => {
    const before = nameTab(split(), ID, "the build");
    const closed = named(
      before,
      closePane(before, ID, second(before), () => "ops"),
    );
    expect(tabNameOf(closed.byId[ID])).toBe("the build");
  });

  it("is never a blank: a blank clears it", () => {
    expect(nameTab(nameTab(one(), ID, "the build"), ID, "  ").byId[ID].own).toBeUndefined();
  });
});

describe("a chat's rename", () => {
  it("names the tab it leads", () => {
    expect(tabNameOf(chatRenamed(one(), 1, "planner").byId[ID])).toBe("planner");
  });

  it("names a split tab it leads, and not one it is only a pane of", () => {
    expect(tabNameOf(chatRenamed(split(), 1, "planner").byId[ID])).toBe("planner");
    expect(tabNameOf(chatRenamed(split(), 2, "builder").byId[ID])).toBe("steward 1");
  });

  it("leaves a tab with its own name alone", () => {
    const tabs = chatRenamed(nameTab(one(), ID, "the build"), 1, "planner");
    expect(tabNameOf(tabs.byId[ID])).toBe("the build");
  });

  it("puts the default back for a name taken off", () => {
    const renamed = chatRenamed(one(), 1, "planner");
    expect(tabNameOf(chatRenamed(renamed, 1, null).byId[ID])).toBe("steward 1");
  });

  it("answers the tabs themselves where no tab is named for that chat", () => {
    const tabs = split();
    expect(chatRenamed(tabs, 2, "builder")).toBe(tabs);
  });
});
