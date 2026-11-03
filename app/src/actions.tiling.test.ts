// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  catalogue,
  catalogued,
  type Doing,
  FOCUS_MODE_ID,
  menuRows,
  type Now,
  type Offer,
  ONE_PANE_TO_TILE,
  perform,
  tileAllId,
  UNDO_LAYOUT_ID,
} from "./actions";
import { noTabs, openTab, splitFocusedPane, type Tabs } from "./tabs";
import { tileAll, zoomFocusedPane } from "./tiling";

/**
 * **Tile all, its Undo and Focus this pane as rows of the one list** (#4765): Tile all on the
 * tab menu and the palette, greyed at one pane; Undo while the tab is as Tile all left it;
 * ⌘⇧Enter's row zooming in a split and staying Focus mode in a one-pane tab.
 */

function now(tabs: Tabs, over: Partial<Now> = {}): Now {
  return { tabs, workspaces: [], needsYou: [], nameOf: (session) => String(session), ...over };
}

const by = (offers: readonly Offer[], id: string) => offers.find((offer) => offer.id === id);

/** The row called `id`, which the test needs there. */
function row(offers: readonly Offer[], id: string): Offer {
  const found = by(offers, id);
  if (found === undefined) throw new Error(`no row called ${id}`);
  return found;
}

/** A `Doing` that records the verbs it is handed, and does nothing else. */
function hands(): { doing: Doing; calls: string[] } {
  const calls: string[] = [];
  const doing = new Proxy({} as Doing, {
    get:
      (_, verb) =>
      (...args: unknown[]) => {
        calls.push([String(verb), ...args].join(":"));
        return { ok: true };
      },
  });
  return { doing, calls };
}

const one = () => openTab(noTabs(), 7, "7", "steward");
const two = () => splitFocusedPane(one(), "row", 8, "8");

describe("Tile all as a row (#4765)", () => {
  it("is greyed with its reason on a tab of one pane", () => {
    const row = by(catalogue(now(one())), tileAllId(1));

    expect(row).toMatchObject({ available: false, reason: ONE_PANE_TO_TILE });
  });

  it("tiles the tab it names, from the palette or that tab's menu", async () => {
    const offers = catalogue(now(two()));
    const { doing, calls } = hands();

    await perform(row(offers, tileAllId(1)), doing);

    expect(calls).toEqual(["tileAll:1"]);
  });

  // **Where it is drawn** (`prototype/pane-chat-actions/command-table.md`, rows 88 and 98):
  // the tab's right-click, the pane head's right-click and the palette. On the tab's menu among
  // its own rows, right after Pin tab (#4760); on the pane's, with the splits.
  it("is on the tab's menu, right after Pin tab", () => {
    const rows = menuRows({ on: "tab", tab: 1 }, catalogued(catalogue(now(two()))));
    const above = rows.above.map((row) => row.id);

    expect(above).toContain("tab.tile:1");
    expect(above.indexOf("tab.tile:1")).toBe(above.indexOf("tab.pin:1") + 1);
    // Move to new window is among the tab's own rows too (#4768).
    expect(above).toContain("tab.newWindow:1");
    expect(rows.below.map((row) => row.id)).not.toContain("tab.tile:1");
  });

  it("is on the pane head's menu, as the tab's own row, after the splits", () => {
    const rows = menuRows({ on: "pane", tab: 1 }, catalogued(catalogue(now(two()))));
    const above = rows.above.map((row) => row.id);

    expect(above).toContain("tab.tile:1");
    expect(above.indexOf("tab.tile:1")).toBeGreaterThan(above.indexOf("pane.split.down"));
    expect(rows.below.map((row) => row.id)).not.toContain("tab.tile:1");
  });

  it("is on no other surface's menu", () => {
    const offers = catalogued(catalogue(now(two())));
    const ids = (on: Parameters<typeof menuRows>[0]) => {
      const rows = menuRows(on, offers);
      return [...rows.above, ...rows.below].map((row) => row.id);
    };

    expect(ids({ on: "chat", session: 1 })).not.toContain("tab.tile:1");
    expect(ids({ on: "root" })).not.toContain("tab.tile:1");
  });
});

describe("Undo after Tile all, as a row (#4765)", () => {
  it("is offered only while the tabs are as Tile all left them", async () => {
    expect(by(catalogue(now(two())), UNDO_LAYOUT_ID)).toBeUndefined();

    const tiled = tileAll(two(), 1);
    const offers = catalogue(now(tiled));
    const { doing, calls } = hands();
    await perform(row(offers, UNDO_LAYOUT_ID), doing);

    expect(calls).toEqual(["undoLayout"]);
    expect(by(catalogue(now(splitFocusedPane(tiled, "row", 9, "9"))), UNDO_LAYOUT_ID)).toBe(
      undefined,
    );
  });
});

describe("Focus this pane, ⌘⇧Enter's row (#4765)", () => {
  it("zooms the focused pane in a split", async () => {
    const focus = row(catalogue(now(two())), FOCUS_MODE_ID);
    const { doing, calls } = hands();
    await perform(focus, doing);

    expect(focus.title).toBe("Focus this pane");
    expect(calls).toEqual(["zoomPane"]);
  });

  it("brings the split back while a pane is zoomed", async () => {
    const focus = row(catalogue(now(zoomFocusedPane(two()))), FOCUS_MODE_ID);
    const { doing, calls } = hands();
    await perform(focus, doing);

    expect(focus.title).toBe("Back to the split");
    expect(calls).toEqual(["zoomPane"]);
  });

  it("is Focus mode in a one-pane tab, and with no tab at all", async () => {
    for (const tabs of [one(), noTabs()]) {
      const focus = row(catalogue(now(tabs)), FOCUS_MODE_ID);
      const { doing, calls } = hands();
      await perform(focus, doing);

      expect(focus.title).toBe("Focus this pane");
      expect(calls).toEqual(["toggleFocusMode"]);
    }
  });

  it("leaves focus mode first, even in a split", async () => {
    const focus = row(catalogue(now(two(), { focusMode: true })), FOCUS_MODE_ID);
    const { doing, calls } = hands();
    await perform(focus, doing);

    expect(focus.title).toBe("Leave focus mode");
    expect(calls).toEqual(["toggleFocusMode"]);
  });
});
