// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { canUndoLayout, recorded, undoLayout } from "./layoutUndo";
import { focusPane, noTabs, openTab, renameTab, splitFocusedPane, type Tabs } from "./tabs";

/**
 * **The window's one Undo** (#4763, #4765): a drop, a split and Tile all each record their
 * change, and the toast, ⌘Z and the palette's row put back the same last one.
 */

const one = (): Tabs => openTab(noTabs(), 7, "7", "steward");
const split = (tabs: Tabs, session: number): Tabs =>
  recorded(tabs, splitFocusedPane(tabs, "row", session, String(session)), "Split the pane.");

describe("Undo after a layout change (#4763)", () => {
  it("records what the change did, and puts the tabs before it back", () => {
    const tabs = one();
    const after = split(tabs, 8);

    expect(after.undo?.did).toBe("Split the pane.");
    expect(canUndoLayout(after)).toBe(true);
    const back = undoLayout(after);
    expect(back.byId[back.order[0]].layout).toBe(tabs.byId[tabs.order[0]].layout);
    expect(back.undo).toBeUndefined();
  });

  it("records nothing for a change that changed nothing", () => {
    const tabs = one();

    expect(recorded(tabs, tabs, "Split the pane.")).toBe(tabs);
  });

  it("is one step for the window: the last change is the one put back", () => {
    const first = split(one(), 8);
    const second = split(first, 9);

    const back = undoLayout(second);

    expect(back.byId[back.order[0]].layout).toBe(first.byId[first.order[0]].layout);
    expect(canUndoLayout(back)).toBe(false);
  });

  it("is gone once the layout changed without it, and not by focus or a rename", () => {
    const after = split(one(), 8);
    const id = after.order[0];

    expect(canUndoLayout(focusPane(after, 1))).toBe(true);
    expect(canUndoLayout(renameTab(after, id, "mine"))).toBe(true);
    const unrecorded = splitFocusedPane(after, "column", 9, "9");
    expect(canUndoLayout(unrecorded)).toBe(false);
    expect(undoLayout(unrecorded)).toBe(unrecorded);
  });
});
