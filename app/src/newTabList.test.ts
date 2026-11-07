// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it, vi } from "vitest";
import { catalogue, perform, type Doing, type Now } from "./actions";
import { keySaid } from "./keys/registry";
import { FILTER_FROM, inANewTab, newTabRows, shown } from "./newTabList";
import { noTabs } from "./tabs";

/**
 * **The strip's `+` list** (#5292; spec #5288 stories 78 to 81): its rows are the catalogue's,
 * each marked where it is built, so a new one joins without a new menu. This pins which rows it
 * holds, in what order, that each key said beside a row is the key that does it, and that a row
 * chosen there opens in a tab of its own.
 */

const PLANE = "/home/dev/plane";

function now(over: Partial<Now> = {}): Now {
  return {
    tabs: noTabs(),
    workspaces: ["alpha"],
    focused: "alpha",
    needsYou: [],
    nameOf: (session) => String(session),
    ...over,
  };
}

describe("the + list's rows (#5292)", () => {
  it("are Empty tab, New chat and New shell first, then the views, from the catalogue", () => {
    const rows = newTabRows(catalogue(now()));

    expect(rows.map((row) => row.said)).toEqual([
      "Empty tab",
      "New chat",
      "New shell",
      "New browser",
      "Extensions…",
      "Keyboard shortcuts",
      "Settings…",
      "Agent identity",
    ]);
    expect(rows.map((row) => row.group)).toEqual([
      "start",
      "start",
      "start",
      "view",
      "view",
      "view",
      "view",
      "view",
    ]);
  });

  it("hold a project's own views while one is open", () => {
    const rows = newTabRows(catalogue(now({ plane: PLANE })));

    expect(rows.map((row) => row.said).slice(8)).toEqual([
      "Shared memory",
      "Dispatches",
      "Browsers",
      "Skill studio",
    ]);
  });

  it("say each key that does what the row does, from the key registry", () => {
    const rows = newTabRows(catalogue(now()));
    const keys = Object.fromEntries(rows.map((row) => [row.said, row.offer.id]));

    expect(keySaid("tab.new", true)).toBe("⌘T");
    expect(keys["Empty tab"]).toBe("tab.new");
    expect(keySaid("chat.new", true)).toBe("⌘N");
    expect(keys["New chat"]).toBe("chat.new");
    expect(keySaid("shell.new", true)).toBe("⌘⇧T");
    expect(keys["New shell"]).toBe("shell.new");
    expect(keys["Settings…"]).toBe("settings.show");
    expect(keys["Keyboard shortcuts"]).toBe("shortcuts.show");
  });

  it("grow with the catalogue: a view it offers is a row", () => {
    const rows = newTabRows(catalogue(now({ plane: PLANE })));

    expect(rows.map((row) => row.said)).toContain("Skill studio");
  });

  it("leave out every row the catalogue does not mark", () => {
    const offers = catalogue(now());
    const rows = newTabRows(offers);

    expect(rows.length).toBeLessThan(offers.length);
    for (const row of rows) expect(row.offer.plus).toBeDefined();
  });
});

describe("the + list's filter (#5292; spec #5288 story 80)", () => {
  it("comes once the list is longer than about eight rows: in a project, not before one", () => {
    expect(FILTER_FROM).toBe(8);
    expect(newTabRows(catalogue(now())).length).toBeLessThanOrEqual(FILTER_FROM);
    expect(newTabRows(catalogue(now({ plane: PLANE }))).length).toBeGreaterThan(FILTER_FROM);
  });

  it("keeps the rows whose words hold what is typed, whatever its case", () => {
    const rows = newTabRows(catalogue(now()));

    expect(shown(rows, "").map((row) => row.said)).toEqual(rows.map((row) => row.said));
    expect(shown(rows, "SHELL").map((row) => row.said)).toEqual(["New shell"]);
    expect(shown(rows, "new").map((row) => row.said)).toEqual([
      "New chat",
      "New shell",
      "New browser",
    ]);
    expect(shown(rows, "nothing like it")).toEqual([]);
  });
});

describe("a row chosen on the + list (#5292, the call from #5297)", () => {
  const hands = () =>
    ({
      newChat: vi.fn(),
      newTab: vi.fn(),
      newShell: vi.fn(),
    }) as unknown as Doing & Record<"newChat" | "newTab" | "newShell", ReturnType<typeof vi.fn>>;
  const row = (said: string) => {
    const found = newTabRows(catalogue(now())).find((one) => one.said === said);
    if (found === undefined) throw new Error(`no row says ${said}`);
    return inANewTab(found.offer);
  };

  it("New chat asks for a tab of its own", () => {
    const doing = hands();
    void perform(row("New chat"), doing);
    expect(doing.newChat).toHaveBeenCalledWith({ ownTab: true });
  });

  it("New shell asks for a tab of its own", () => {
    const doing = hands();
    void perform(row("New shell"), doing);
    expect(doing.newShell).toHaveBeenCalledWith(undefined, { ownTab: true });
  });

  it("Empty tab is the strip's own +", () => {
    const doing = hands();
    void perform(row("Empty tab"), doing);
    expect(doing.newTab).toHaveBeenCalledWith();
  });

  it("leaves the sidebar's New chat to the one rule, which fills an empty pane first", () => {
    const doing = hands();
    const offer = catalogue(now()).find((one) => one.id === "chat.new");
    if (offer === undefined) throw new Error("no New chat");
    void perform(offer, doing);
    expect(doing.newChat).toHaveBeenCalledWith();
  });
});
