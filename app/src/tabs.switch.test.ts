// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { EMPTY_TAB, noTabs, openEmptyTab, openTab, press, workspaceOf, type Tabs } from "./tabs";

/**
 * **The switch: a chat isn't a tab** (#4758). The strip's `+` opens a tab with one empty pane
 * (spec story 13), and a chat the person starts lands where a press would (story 12).
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

  it("is a tab New chat then fills, adding no other", () => {
    const tabs: Tabs = openEmptyTab(openTab(noTabs(), 1, "1", "steward"), "ops");

    const landed = press(tabs, { session: 2, chat: "2", who: "steward" }, at());

    expect(landed.tabs.order).toEqual(tabs.order);
    const front = landed.tabs.byId[landed.tabs.inFront ?? -1];
    expect(front.layout).toMatchObject({ kind: "pane", content: { session: 2 } });
  });
});
