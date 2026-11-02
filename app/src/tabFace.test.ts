// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { faceOf, type Naming } from "./tabFace";
import { focusPane, noTabs, openTab, openView, splitFocusedPane, type Tabs } from "./tabs";

/**
 * **What a tab wears on the strip** (#4762, `prototype/one-tab-strip` variant A): its focused
 * pane's combined icon and name, `⊟+N` for the panes beside it, and every pane's name on hover.
 */

/** Chats by number, as the sidebar names them. */
const names: Record<number, string> = { 1: "steward 1", 2: "devops 2", 3: "talk" };
const naming: Naming = {
  chat: (session) => names[session] ?? String(session),
  view: () => "Settings",
};

function front(tabs: Tabs): number {
  if (tabs.inFront === undefined) throw new Error("no tab is in front");
  return tabs.inFront;
}

describe("a tab's face", () => {
  it("is its one chat's icon and name", () => {
    const tabs = openTab(noTabs(), 1);
    expect(faceOf(tabs, front(tabs), naming)).toEqual({
      lead: { kind: "chat", session: 1 },
      name: "steward 1",
      more: 0,
      panes: ["steward 1"],
    });
  });

  it("is the focused pane's in a split, with how many panes are beside it", () => {
    const tabs = splitFocusedPane(splitFocusedPane(openTab(noTabs(), 1), "row", 2), "column", 3);
    const face = faceOf(tabs, front(tabs), naming);
    expect(face.lead).toEqual({ kind: "chat", session: 3 });
    expect(face.name).toBe("talk");
    expect(face.more).toBe(2);
  });

  it("follows the focus to another pane", () => {
    const split = splitFocusedPane(openTab(noTabs(), 1), "row", 2);
    const id = front(split);
    const first = split.byId[id].layout;
    if (first.kind !== "split" || first.children[0].kind !== "pane") throw new Error("a split");
    const tabs = focusPane(split, first.children[0].pane);
    expect(faceOf(tabs, id, naming).name).toBe("steward 1");
  });

  it("lists every pane, in reading order, for the tooltip", () => {
    const tabs = splitFocusedPane(splitFocusedPane(openTab(noTabs(), 1), "row", 2), "column", 3);
    expect(faceOf(tabs, front(tabs), naming).panes).toEqual(["steward 1", "devops 2", "talk"]);
  });

  it("shows a view as the view, with no chat state", () => {
    const tabs = openView(
      noTabs(),
      { from: null, view: "settings", key: "project" },
      "Settings",
      "alpha",
    );
    expect(faceOf(tabs, front(tabs), naming)).toMatchObject({
      lead: { kind: "view", view: { view: "settings" } },
      name: "Settings",
      more: 0,
    });
  });

  it("says New chat for a tab that is not there to read", () => {
    // An empty pane's tab, and the frame between a close and the strip following it.
    expect(faceOf(noTabs(), 9, naming)).toEqual({
      lead: { kind: "empty" },
      name: "New chat",
      more: 0,
      panes: ["New chat"],
    });
  });
});
