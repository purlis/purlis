// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { faceOf, type Naming } from "./tabFace";
import {
  contentsOf,
  focusPane,
  noTabs,
  openTab,
  openView,
  openViewBesidePane,
  showInstead,
  splitFocusedPane,
  type Tabs,
  type ViewRef,
} from "./tabs";
import { zoomFocusedPane } from "./tiling";

/**
 * **What a tab wears on the strip** (#4762, `prototype/one-tab-strip` variant A, as #5342
 * amends it): its first pane's name, every pane's chat for its mark to choose the most urgent
 * of, `⊟+N` for the panes beside it, and every pane's name on hover.
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
    const tabs = openTab(noTabs(), 1, "steward 1");
    expect(faceOf(tabs, front(tabs), naming)).toEqual({
      lead: { kind: "chat", session: 1 },
      name: "steward 1",
      chats: [1],
      panes: ["steward 1"],
    });
  });

  it("is its first pane's in a split, whichever pane is focused, with how many are beside it", () => {
    // Splitting doesn't rename the tab (#5342, amending #4762): the focus is on talk.
    const tabs = splitFocusedPane(
      splitFocusedPane(openTab(noTabs(), 1, "steward 1"), "row", 2),
      "column",
      3,
    );
    const face = faceOf(tabs, front(tabs), naming);
    expect(face.lead).toEqual({ kind: "chat", session: 1 });
    expect(face.name).toBe("steward 1");
    expect(face.picture?.panes).toBe(3);
  });

  it("offers every pane's chat to its mark, in reading order (#5342)", () => {
    const tabs = splitFocusedPane(
      splitFocusedPane(openTab(noTabs(), 1, "steward 1"), "row", 2),
      "column",
      3,
    );
    expect(faceOf(tabs, front(tabs), naming).chats).toEqual([1, 2, 3]);
  });

  it("offers a task shown in a pane's place to its mark, after the pane's own chat (#5342)", () => {
    const split = splitFocusedPane(openTab(noTabs(), 1, "steward 1"), "row", 2);
    const id = front(split);
    const second = split.byId[id].focused;
    const tabs: Tabs = {
      ...split,
      byId: { ...split.byId, [id]: { ...split.byId[id], shows: { [second]: 7 } } },
    };
    expect(faceOf(tabs, id, naming).chats).toEqual([1, 2, 7]);
  });

  it("is named by the tab's own name, and its first pane's chat stays its own (#5342)", () => {
    const split = splitFocusedPane(openTab(noTabs(), 1, "steward 1"), "row", 2);
    const id = front(split);
    const tabs: Tabs = {
      ...split,
      byId: { ...split.byId, [id]: { ...split.byId[id], own: "the build" } },
    };
    expect(faceOf(tabs, id, naming)).toMatchObject({
      name: "the build",
      panes: ["steward 1", "devops 2"],
    });
  });

  it("wears the zoom mark while a pane is zoomed, and not once the split is back (#4765)", () => {
    const split = splitFocusedPane(openTab(noTabs(), 1, "steward 1"), "row", 2);
    const zoomed = zoomFocusedPane(split);

    expect(faceOf(split, front(split), naming).zoomed).toBeUndefined();
    expect(faceOf(zoomed, front(zoomed), naming)).toMatchObject({
      zoomed: true,
      picture: { panes: 2 },
    });
    expect(faceOf(zoomFocusedPane(zoomed), front(zoomed), naming).zoomed).toBeUndefined();
  });

  it("stays its first pane's as the focus moves between panes (#5342)", () => {
    const split = splitFocusedPane(openTab(noTabs(), 1, "steward 1"), "row", 2);
    const id = front(split);
    const first = split.byId[id].layout;
    if (first.kind !== "split" || first.children[0].kind !== "pane") throw new Error("a split");
    for (const pane of [first.children[0].pane, split.byId[id].focused]) {
      expect(faceOf(focusPane(split, pane), id, naming)).toMatchObject({
        lead: { kind: "chat", session: 1 },
        name: "steward 1",
      });
    }
  });

  it("lists every pane, in reading order, for the tooltip", () => {
    const tabs = splitFocusedPane(
      splitFocusedPane(openTab(noTabs(), 1, "steward 1"), "row", 2),
      "column",
      3,
    );
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
    });
  });

  it("names its first pane by the tab's own name, which a rename sets", () => {
    const tabs = openTab(noTabs(), 1, "the build");
    expect(faceOf(tabs, front(tabs), naming).name).toBe("the build");
    expect(faceOf(splitFocusedPane(tabs, "row", 2), front(tabs), naming).panes).toEqual([
      "the build",
      "devops 2",
    ]);
  });

  it("names a view pane with no name of its own by its tab, as DevTools beside a browser", () => {
    const tabs = splitFocusedPane(openTab(noTabs(), 1, "steward 1"), "row", 2);
    const unnamed: Naming = { ...naming, chat: () => "devops 2", view: () => undefined };
    const browser = openView(
      noTabs(),
      { from: null, view: "browser", key: "B1" },
      "Browser",
      "alpha",
    );
    const id = front(browser);
    const lead = browser.byId[id].layout;
    if (lead.kind !== "pane") throw new Error("one pane");
    const beside: Tabs = {
      ...browser,
      byId: {
        ...browser.byId,
        [id]: {
          ...browser.byId[id],
          focused: 99,
          layout: {
            kind: "split",
            direction: "row",
            children: [
              lead,
              {
                kind: "pane",
                pane: 99,
                content: {
                  kind: "view",
                  view: { from: null, view: "devtools", key: "B1" },
                  workspace: "alpha",
                },
              },
            ],
          },
        },
      },
    };
    expect(faceOf(beside, id, unnamed).name).toBe("Browser");
    expect(faceOf(tabs, front(tabs), unnamed).name).toBe("steward 1");
  });

  it("names a view beside a chat by its own title, which the tab takes when it is first (#5577)", () => {
    // Opened beside chat 1 from a file's row: the view's own title is the file's name.
    const file: ViewRef = { from: null, view: "file", key: "/repo/notes.md" };
    const tabs = openViewBesidePane(
      openTab(noTabs(), 1, "steward 1"),
      1,
      file,
      "notes.md",
      "alpha",
      "row",
    );
    const untitled: Naming = { ...naming, view: () => undefined };
    expect(faceOf(tabs, front(tabs), untitled).panes).toEqual(["steward 1", "notes.md"]);
    // Its first chat's pane closed: the tab is named for the view, by its title.
    const id = front(tabs);
    const content = contentsOf(tabs, id)[1].content;
    expect(content.kind === "view" && content.title).toBe("notes.md");
  });

  it("keeps a view's own title current when the view is shown anew in its pane (#5577)", () => {
    const file: ViewRef = { from: null, view: "file", key: "/repo/notes.md" };
    const moved: ViewRef = { from: null, view: "file", key: "/repo/later.md" };
    const tabs = showInstead(
      openViewBesidePane(openTab(noTabs(), 1, "steward 1"), 1, file, "notes.md", "alpha", "row"),
      file,
      moved,
      "later.md",
    );
    const untitled: Naming = { ...naming, view: () => undefined };
    expect(faceOf(tabs, front(tabs), untitled).panes).toEqual(["steward 1", "later.md"]);
  });

  it("says New chat for a tab that is not there to read", () => {
    // An empty pane's tab, and the frame between a close and the strip following it.
    expect(faceOf(noTabs(), 9, naming)).toEqual({
      lead: { kind: "empty" },
      name: "New chat",
      chats: [],
      panes: ["New chat"],
    });
  });
});
