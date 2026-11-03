// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  besideHint,
  besideLabel,
  besideOf,
  besideRefusedSaid,
  openedBesideSaid,
} from "./viewBeside";
import { PANE_FLOOR } from "./paneDrop";
import {
  contentsOf,
  findView,
  noTabs,
  openTab,
  openView,
  openViewBesidePane,
  type Tabs,
  type ViewRef,
} from "./tabs";

/**
 * **Open beside, judged before the press** (#4769, prototype `view-open-beside` Q2 A): the
 * split follows the pane's shape, and a split that would leave a side under about 40 columns by
 * 8 rows (#4583's floor, `PANE_FLOOR`) is refused with its reason, which the beside icon, the
 * menu row and the palette's hint say before anything is pressed.
 */
describe("where a view opens beside a pane (#4769)", () => {
  it("splits a wide pane side by side and a tall one stacked", () => {
    expect(besideOf({ width: 1000, height: 600 })).toEqual({ ok: true, direction: "row" });
    expect(besideOf({ width: 600, height: 1000 })).toEqual({ ok: true, direction: "column" });
  });

  it("is refused when a side would be under about 40 columns, saying so", () => {
    const narrow = besideOf({ width: PANE_FLOOR.width * 2 - 2, height: 100 });
    expect(narrow).toEqual({
      ok: false,
      direction: "row",
      why: "Too narrow to open beside: each side needs 40 columns",
    });
  });

  it("is refused when a side would be under about 8 rows, saying so", () => {
    const short = besideOf({ width: 200, height: PANE_FLOOR.height * 2 - 2 });
    expect(short).toEqual({
      ok: false,
      direction: "column",
      why: "Too short to open beside: each side needs 8 rows",
    });
  });

  it("opens beside at exactly the floor", () => {
    expect(besideOf({ width: PANE_FLOOR.width * 2, height: 100 }).ok).toBe(true);
  });

  it("splits side by side where the pane's size is not known yet", () => {
    expect(besideOf(undefined)).toEqual({ ok: true, direction: "row" });
  });
});

describe("what Open beside says (#4769)", () => {
  it("labels the beside icon by where it opens, and by its reason when refused", () => {
    const ok = besideOf({ width: 1000, height: 600 });
    expect(besideLabel("Statistics · steward", "this", ok)).toBe(
      "Open Statistics · steward beside this pane (⌥-press)",
    );
    expect(besideLabel("Statistics · steward", "focused", ok)).toBe(
      "Open Statistics · steward beside the focused pane (⌥-press)",
    );
    const refused = besideOf({ width: 400, height: 100 });
    expect(besideLabel("Statistics · steward", "this", refused)).toBe(
      "Too narrow to open beside: each side needs 40 columns",
    );
  });

  it("says a refusal after an ⌥-press as one sentence with its reason", () => {
    const refused = besideOf({ width: 400, height: 100 });
    expect(refused.ok).toBe(false);
    if (refused.ok) return;
    expect(besideRefusedSaid(refused)).toBe("Not opened beside: each side needs 40 columns.");
  });

  it("says the palette's two keys on a view's row, and the reason in ⌥'s place", () => {
    expect(besideHint(besideOf({ width: 1000, height: 600 }), true)).toBe(
      "↩ Open in a tab · ⌥↩ Open beside the focused pane",
    );
    expect(besideHint(besideOf({ width: 400, height: 100 }), false)).toBe(
      "Enter Open in a tab · Alt+Enter Too narrow to open beside: each side needs 40 columns",
    );
  });

  it("says where it opened, for the reader, by the split's direction", () => {
    expect(openedBesideSaid("Statistics · steward", "steward 4", "row")).toBe(
      "Opened Statistics · steward right of steward 4. Focus is in it.",
    );
    expect(openedBesideSaid("Statistics · steward", "steward 4", "column")).toBe(
      "Opened Statistics · steward below steward 4. Focus is in it.",
    );
  });
});

const STATS: ViewRef = { from: "statistics", view: "stats", key: "steward" };
const PERSONA: ViewRef = { from: null, view: "persona", key: "steward" };

/** Chat 1 in a tab, and steward's card in a tab of its own, which is in front. */
function chatAndCard(): { tabs: Tabs; chatTab: number; chatPane: number; cardPane: number } {
  const tabs = openView(openTab(noTabs(), 1, "1", "steward"), PERSONA, "steward", "ops");
  const [chatTab, cardTab] = tabs.order;
  return {
    tabs,
    chatTab,
    chatPane: tabs.byId[chatTab].focused,
    cardPane: tabs.byId[cardTab].focused,
  };
}

describe("a view opened beside a pane (#4769)", () => {
  it("splits that pane in its tab, brings the tab forward and puts focus in the view", () => {
    const { tabs, chatTab, chatPane } = chatAndCard();
    const after = openViewBesidePane(tabs, chatPane, STATS, "Statistics · steward", "ops", "row");
    const tab = after.byId[chatTab];
    expect(after.inFront).toBe(chatTab);
    expect(tab.layout).toMatchObject({
      kind: "split",
      direction: "row",
      children: [
        { kind: "pane", pane: chatPane },
        { kind: "pane", content: { kind: "view", view: STATS } },
      ],
    });
    expect(tab.focused).not.toBe(chatPane);
    expect(findView(after, STATS)).toEqual({ tab: chatTab, pane: tab.focused });
  });

  it("stacks it below a pane split by its shape", () => {
    const { tabs, chatTab, chatPane } = chatAndCard();
    const after = openViewBesidePane(tabs, chatPane, STATS, "Statistics", "ops", "column");
    expect(after.byId[chatTab].layout).toMatchObject({ kind: "split", direction: "column" });
  });

  it("brings a view already open forward, never opening it twice", () => {
    const { tabs, chatPane, cardPane } = chatAndCard();
    const once = openViewBesidePane(tabs, cardPane, STATS, "Statistics", "ops", "row");
    const twice = openViewBesidePane(once, chatPane, STATS, "Statistics", "ops", "row");
    const where = findView(once, STATS);
    expect(findView(twice, STATS)).toEqual(where);
    expect(twice.inFront).toBe(where?.tab);
    expect(twice.byId[where?.tab ?? -1].focused).toBe(where?.pane);
    // No second pane anywhere shows it.
    const showing = twice.order.flatMap((id) =>
      contentsOf(twice, id).filter(
        ({ content }) => content.kind === "view" && content.view.view === STATS.view,
      ),
    );
    expect(showing).toHaveLength(1);
  });

  it("opens it in a tab of its own where the pane has closed", () => {
    const { tabs } = chatAndCard();
    const after = openViewBesidePane(tabs, 999, STATS, "Statistics", "ops", "row");
    const at = findView(after, STATS);
    expect(at).toBeDefined();
    expect(after.order).toHaveLength(3);
    expect(after.inFront).toBe(at?.tab);
  });
});
