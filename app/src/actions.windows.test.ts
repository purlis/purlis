// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  catalogue,
  catalogued,
  type Doing,
  inPalette,
  menuRows,
  type Now,
  type Offer,
  PANE_TO_NEW_WINDOW,
  paneToWindowId,
  perform,
  toNewWindowId,
  toWindowId,
} from "./actions";
import { menuOf, type MenuDrawn } from "./menuModel";
import { noTabs, openTab, selectTab, showIn, type Tabs } from "./tabs";

/**
 * **Move to new window and Move to window ▸ as rows of the one list** (#4768, ruled on #4701):
 * on a tab's menu, on the panes' and in the palette, so no move lives on one surface only.
 */

function now(tabs: Tabs, over: Partial<Now> = {}): Now {
  return { tabs, workspaces: [], needsYou: [], nameOf: (session) => String(session), ...over };
}

const by = (offers: readonly Offer[], id: string) => offers.find((offer) => offer.id === id);

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

/** Two tabs, the first in front. */
const two = () => {
  const tabs = openTab(openTab(noTabs(), 7, "7", "steward"), 8, "8", "steward");
  return selectTab(tabs, tabs.order[0]);
};

/** What a menu says, each group's rows and each submenu's as `name ▸ [rows]`. */
function said(menu: MenuDrawn): string[] {
  return menu.groups.flatMap((group) =>
    group.entries.map((entry) =>
      entry.kind === "row"
        ? entry.said
        : `${entry.said} ▸ ${entry.groups.flatMap((one) => one.entries.map((row) => row.said)).join(", ")}`,
    ),
  );
}

describe("Move to new window and Move to window ▸ (#4768)", () => {
  it("are on a tab's menu, Move to window listing each other window of the project", () => {
    const offers = catalogued(catalogue(now(two(), { otherWindows: [0, 2] })));

    const menu = said(menuOf({ on: "tab", tab: 2 }, offers));

    expect(menu).toContain("Move to new window");
    expect(menu).toContain("Move to window ▸ Project window, Tab window 2");
  });

  it("offer no Move to window in a project of one window", () => {
    const offers = catalogued(catalogue(now(two())));

    const menu = said(menuOf({ on: "tab", tab: 1 }, offers));

    expect(menu).toContain("Move to new window");
    expect(menu.some((row) => row.startsWith("Move to window"))).toBe(false);
  });

  it("are in the palette for the tab in front, and on each tab's own menu for the rest", () => {
    const offers = catalogue(now(two(), { otherWindows: [0] }));
    const listed = inPalette(offers).map((offer) => offer.id);

    expect(listed).toContain(toNewWindowId(1));
    expect(listed).toContain(toWindowId(1, 0));
    expect(listed).not.toContain(toNewWindowId(2));
    expect(by(offers, toNewWindowId(2))?.title).toBe("Move tab steward 8 to new window");
    expect(by(offers, toWindowId(2, 0))?.title).toBe("Move tab steward 8 to the project window");
  });

  it("move the tab they name, and nothing ends", async () => {
    const offers = catalogue(now(two(), { otherWindows: [3] }));
    const { doing, calls } = hands();

    await perform(by(offers, toNewWindowId(2)) as Offer, doing);
    await perform(by(offers, toWindowId(1, 3)) as Offer, doing);

    expect(calls).toEqual(["moveToWindow:2:false:", "moveToWindow:1:false:3"]);
  });

  it("move the focused pane from the panes' menu and the palette, and an empty pane is greyed with why", async () => {
    const offers = catalogue(now(two(), { otherWindows: [0] }));
    const { doing, calls } = hands();

    expect(said(menuOf({ on: "pane" }, catalogued(offers)))).toEqual(
      expect.arrayContaining(["Move to new window", "Move to window ▸ Project window"]),
    );
    expect(inPalette(offers).map((offer) => offer.id)).toContain(PANE_TO_NEW_WINDOW);
    await perform(by(offers, paneToWindowId(0)) as Offer, doing);
    expect(calls).toEqual(["moveToWindow:1:true:0"]);

    const empty = showIn(two(), 1, 1, { kind: "empty", workspace: "ops" });
    const refused = by(catalogue(now(empty)), PANE_TO_NEW_WINDOW);
    expect(refused).toMatchObject({
      available: false,
      reason: "An empty pane has nothing to move.",
    });
  });

  it("put Move to new window on the tab's menu after its own rows, above the line", () => {
    const rows = menuRows({ on: "tab", tab: 1 }, catalogued(catalogue(now(two()))));

    expect(rows.above.map((row) => row.id).at(-1)).toBe(toNewWindowId(1));
    expect(rows.below.map((row) => row.id)).not.toContain(toNewWindowId(1));
  });
});
