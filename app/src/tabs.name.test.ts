// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { recorded, undoLayout } from "./layoutUndo";
import { dropOn, type DropContext } from "./paneDrop";
import {
  closePane,
  focusPane,
  insertPane,
  namedForFirstPanes,
  noTabs,
  openTab,
  panes,
  splitFocusedPane,
  tabNameOf,
  type Content,
  type Tabs,
} from "./tabs";

/**
 * **A tab's name** (#5342, spec #5339): its own name if the person gave it one, otherwise its
 * first pane's, the layout's first leaf at top left. Splitting and moving focus don't change it;
 * when the first chat leaves, the tab takes the next one's; a chat put left of the first pane
 * becomes first; Undo gives the name back with the layout; a name the person gave stays.
 */

/** Chats by number, as their rows name them. */
const names: Record<number, string> = { 1: "steward 1", 2: "devops 2", 3: "talk" };
const nameOf = (content: Content) =>
  content.kind === "session" ? (names[content.session] ?? String(content.session)) : "New chat";
const ctx: DropContext = { filedIn: () => "ops", name: nameOf };
/** What the window does after every change: a tab whose first pane is another names it so. */
const named = (before: Tabs, after: Tabs) =>
  namedForFirstPanes(before, after, (content) => {
    const name = nameOf(content);
    return { name, defaultName: name };
  });

const ID = 1;
const one = () => openTab(noTabs(), 1, "1", "steward");
const paneOf = (tabs: Tabs, session: number) => {
  const found = panes(tabs.byId[ID].layout).find(
    (pane) => pane.content.kind === "session" && pane.content.session === session,
  );
  if (found === undefined) throw new Error(`no pane shows chat ${session}`);
  return found.pane;
};
const split = () => named(one(), splitFocusedPane(one(), "row", 2, "2"));
const ownNamed = (tabs: Tabs): Tabs => ({
  ...tabs,
  byId: { ...tabs.byId, [ID]: { ...tabs.byId[ID], own: "the build" } },
});
const closed = (tabs: Tabs, session: number) =>
  named(tabs, closePane(tabs, ID, paneOf(tabs, session), () => "ops"));

const table: readonly [string, () => Tabs, string][] = [
  ["one pane is its chat's", one, "steward 1"],
  ["a split keeps the first chat's", split, "steward 1"],
  ["moving focus to the second pane keeps it", () => focusPane(split(), paneOf(split(), 2)), "steward 1"],
  ["the first chat leaving gives it the next one's", () => closed(split(), 1), "devops 2"],
  ["the second chat leaving keeps it", () => closed(split(), 2), "steward 1"],
  [
    "a chat split in left of the first pane becomes first",
    () => {
      const before = one();
      const at = { tab: ID, pane: paneOf(before, 1), direction: "row" as const };
      const content: Content = { kind: "session", session: 3, chat: "3" };
      return named(before, insertPane(before, { ...at, side: "before", content }));
    },
    "talk",
  ],
  [
    "a chat dropped left of the first pane becomes first",
    () => {
      const before = split();
      return dropOn(
        before,
        { kind: "pane", tab: ID, pane: paneOf(before, 2) },
        { kind: "pane", tab: ID, pane: paneOf(before, 1), place: "left" },
        ctx,
      );
    },
    "devops 2",
  ],
  [
    "Undo of a close gives the first chat's name back with the layout",
    () => {
      const before = split();
      return undoLayout(recorded(before, closed(before, 1), "Closed the pane."));
    },
    "steward 1",
  ],
  ["its own name is its name", () => ownNamed(one()), "the build"],
  ["its own name stays through a split", () => named(ownNamed(one()), splitFocusedPane(ownNamed(one()), "row", 2)), "the build"],
  ["its own name stays when the first chat leaves", () => closed(ownNamed(split()), 1), "the build"],
];

describe("a tab's name", () => {
  it.each(table)("%s", (_, tabs, name) => {
    expect(tabNameOf(tabs().byId[ID])).toBe(name);
  });
});
