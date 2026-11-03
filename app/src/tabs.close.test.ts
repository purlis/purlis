// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  closeFocusedPane,
  closeLayout,
  contentsOf,
  EMPTY_TAB,
  focusPane,
  type FiledIn,
  noTabs,
  openTab,
  openTabFilling,
  splitFocusedPane,
  type Tabs,
  visibleSessions,
} from "./tabs";

/**
 * **Close never ends a chat** (#4756): Close pane clears a pane and Close tab closes a layout.
 * The window's pure layout model: what is left on the strip. Which chats end is nothing of
 * this model's, since a close ends none (`PlaneView`, `actions.ts`).
 */

/** Chats 11 and 12 work in `alpha`, 33 in `beta`. */
const filed: FiledIn = (session) => (session === 33 ? "beta" : "alpha");

function front(tabs: Tabs): number {
  if (tabs.inFront === undefined) throw new Error("no tab is in front");
  return tabs.inFront;
}

describe("closing a pane (#4756)", () => {
  it("collapses the split, and the chat that was in it is in no pane", () => {
    const tabs = splitFocusedPane(openTab(noTabs(), 11), "row", 12);

    const left = closeFocusedPane(tabs, filed);

    expect(visibleSessions(left)).toEqual([11]);
    expect(left.order).toEqual(tabs.order);
  });

  it("closes a tab's only pane with its tab when the strip has another tab", () => {
    const tabs = openTab(openTab(noTabs(), 11), 12);

    const left = closeFocusedPane(tabs, filed);

    expect(left.order).toEqual([tabs.order[0]]);
    expect(visibleSessions(left)).toEqual([11]);
  });

  it("keeps the strip's last tab, with one empty pane in it, focused", () => {
    const tabs = openTab(noTabs(), 11);

    const left = closeFocusedPane(tabs, filed);

    expect(left.order).toEqual(tabs.order);
    expect(left.inFront).toBe(tabs.inFront);
    const [only] = contentsOf(left, front(left));
    expect(only.content).toEqual({ kind: "empty", workspace: "alpha" });
    expect(left.byId[front(left)].focused).toBe(only.pane);
    expect(left.byId[front(left)].name).toBe(EMPTY_TAB);
    expect(visibleSessions(left)).toEqual([]);
  });

  it("counts only its own strip: a tab on another workspace's strip does not keep it", () => {
    const tabs = openTab(openTab(noTabs(), 33), 11);

    const left = closeFocusedPane(tabs, filed);

    expect(left.order).toEqual(tabs.order);
    expect(contentsOf(left, front(left))[0].content).toEqual({ kind: "empty", workspace: "alpha" });
  });

  it("gives the empty pane an id of its own, so it is never confused with the pane that closed", () => {
    const tabs = openTab(noTabs(), 11);
    const was = contentsOf(tabs, front(tabs))[0].pane;

    const left = closeFocusedPane(tabs, filed);

    expect(contentsOf(left, front(left))[0].pane).not.toBe(was);
  });
});

describe("closing a tab (#4756)", () => {
  it("closes the layout and brings the tab beside it to the front", () => {
    const tabs = openTab(splitFocusedPane(openTab(noTabs(), 11), "row", 12), 12 + 1);

    const left = closeLayout(tabs, front(tabs), filed);

    expect(left.order).toEqual([tabs.order[0]]);
    expect(visibleSessions(left)).toEqual([11, 12]);
  });

  it("keeps the strip's last tab, with one empty pane in place of its whole layout", () => {
    const tabs = splitFocusedPane(openTab(noTabs(), 11), "row", 12);

    const left = closeLayout(tabs, front(tabs), filed);

    expect(left.order).toEqual(tabs.order);
    expect(contentsOf(left, front(left)).map((one) => one.content)).toEqual([
      { kind: "empty", workspace: "alpha" },
    ]);
  });

  it("hands back the very tabs for a strip's last tab that is already one empty pane", () => {
    const empty = closeFocusedPane(openTab(noTabs(), 11), filed);

    expect(closeLayout(empty, front(empty), filed)).toBe(empty);
  });

  it("leaves tabs alone for a tab that is not there", () => {
    const tabs = openTab(noTabs(), 11);

    expect(closeLayout(tabs, 404, filed)).toBe(tabs);
  });
});

describe("a new chat on a strip whose one tab is empty (#4756)", () => {
  it("fills that empty tab rather than opening another beside it", () => {
    const empty = closeFocusedPane(openTab(noTabs(), 11), filed);
    const id = front(empty);

    const opened = openTabFilling(empty, "alpha", 12, "12", "steward");

    expect(opened.order).toEqual([id]);
    expect(opened.inFront).toBe(id);
    expect(visibleSessions(opened)).toEqual([12]);
    expect(opened.byId[id].name).toBe("steward 12");
  });

  it("opens a tab of its own where the empty tab is another strip's", () => {
    const empty = closeFocusedPane(openTab(noTabs(), 11), filed);

    const opened = openTabFilling(empty, "beta", 33, "33", "steward");

    expect(opened.order).toHaveLength(2);
    expect(visibleSessions(opened)).toEqual([33]);
  });

  it("opens a tab of its own where no tab is empty", () => {
    const tabs = openTab(noTabs(), 11);

    expect(openTabFilling(tabs, "alpha", 12, "12", "steward")).toEqual(
      openTab(tabs, 12, "12", "steward"),
    );
  });
});

describe("where the keyboard goes on Close pane (#4756, spec #4750)", () => {
  /** A | (B / C): A on the left, B over C on the right, C focused. */
  function threePanes() {
    const tabs = splitFocusedPane(splitFocusedPane(openTab(noTabs(), 11), "row", 12), "column", 13);
    const [a, b, c] = contentsOf(tabs, front(tabs)).map((one) => one.pane);
    return { tabs, a, b, c };
  }

  it("goes to the pane that takes the space, not the layout's first pane", () => {
    const { tabs, b } = threePanes();

    const left = closeFocusedPane(tabs, filed);

    expect(left.byId[front(left)].focused).toBe(b);
  });

  it("goes to the pane below when the pane above it closes", () => {
    const { tabs, b, c } = threePanes();
    const onB = focusPane(tabs, b);

    const left = closeFocusedPane(onB, filed);

    expect(left.byId[front(left)].focused).toBe(c);
  });
});
