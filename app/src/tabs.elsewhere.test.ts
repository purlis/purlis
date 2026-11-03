// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { OUTSIDE } from "./actions";
import {
  type Content,
  contentsOf,
  filedElsewhere,
  insertPane,
  noTabs,
  openTab,
  press,
  type Tabs,
} from "./tabs";

/**
 * **A pane showing another workspace's chat names that workspace** (#4982; #4583 rule 2): a
 * drag may put a chat of another workspace in a pane, the chat stays filed in its own, and its
 * pane head wears that workspace's badge. This is the model's half: which workspace, if any,
 * a pane's chat is filed in that is not its tab's. The window's pure layout model, beside
 * `tabs.press.test.ts`.
 */

const chat = (session: number): Content => ({ kind: "session", session, chat: String(session) });

/** Chats 1 to 8 work in `ops`, 9 to 19 in `web`, and 20 and up at the plane root. */
const filedIn = (session: number) => (session < 9 ? "ops" : session < 20 ? "web" : OUTSIDE);

/** Chat 1's tab, on `ops`, split with chat `beside` in a second pane. */
function besideOne(beside: number): { tabs: Tabs; id: number } {
  const one = openTab(noTabs(), 1, "1", "steward");
  const [id] = one.order;
  const tabs = insertPane(one, {
    tab: id,
    pane: contentsOf(one, id)[0].pane,
    direction: "row",
    side: "after",
    content: chat(beside),
  });
  return { tabs, id };
}

describe("the workspace a pane's chat is filed in, where it is not its tab's (#4982)", () => {
  it("names the other workspace for a chat of another workspace in the tab", () => {
    const { tabs, id } = besideOne(9);

    expect(filedElsewhere(tabs, id, 9, filedIn)).toBe("web");
  });

  it("names nothing for a chat of the tab's own workspace", () => {
    const { tabs, id } = besideOne(2);

    expect(filedElsewhere(tabs, id, 1, filedIn)).toBeUndefined();
    expect(filedElsewhere(tabs, id, 2, filedIn)).toBeUndefined();
  });

  it("names the plane root for a chat working in no workspace", () => {
    const { tabs, id } = besideOne(20);

    expect(filedElsewhere(tabs, id, 20, filedIn)).toBe(OUTSIDE);
  });

  it("reads the tab's own workspace, not its first pane's, once a press gave it one", () => {
    // Chat 1's tab on `ops`; a press aimed at its only pane (as a breadcrumb walking up to an
    // asker filed elsewhere is) shows web's chat 9 there, and the tab stays on `ops` (#4757):
    // that pane is the one that names `web`.
    const one = openTab(noTabs(), 1, "1", "steward");
    const [id] = one.order;
    const { tabs } = press(
      one,
      { session: 9, chat: "9", who: "steward" },
      { filedIn, frontOf: () => undefined },
      { tab: id, pane: contentsOf(one, id)[0].pane },
    );

    expect(contentsOf(tabs, id).map((at) => at.content)).toEqual([chat(9)]);
    expect(filedElsewhere(tabs, id, 9, filedIn)).toBe("web");
  });

  it("reads a tab no press gave a workspace as its first pane's, as the strip files it", () => {
    // Web's chat 9 first, then ops's chat 2 beside it: the tab is on web's strip, so it is chat
    // 2, not chat 9, that is filed elsewhere.
    const one = openTab(noTabs(), 9, "9", "steward");
    const [id] = one.order;
    const tabs = insertPane(one, {
      tab: id,
      pane: contentsOf(one, id)[0].pane,
      direction: "row",
      side: "after",
      content: chat(2),
    });

    expect(filedElsewhere(tabs, id, 9, filedIn)).toBeUndefined();
    expect(filedElsewhere(tabs, id, 2, filedIn)).toBe("ops");
  });

  it("names nothing for a tab the model does not hold", () => {
    expect(filedElsewhere(noTabs(), 7, 9, filedIn)).toBeUndefined();
  });
});
