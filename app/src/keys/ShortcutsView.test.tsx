// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { ShortcutsView, sheetRows } from "./ShortcutsView";
import { SHORTCUTS } from "./registry";

afterEach(cleanup);

describe("the cheat sheet (N25)", () => {
  it("is generated from the registry: every act with a key on this keyboard, once", () => {
    for (const mac of [true, false]) {
      const rows = sheetRows(mac);
      const ids = rows.map((row) => row.id);
      expect(new Set(ids).size).toBe(ids.length);
      const keyed = SHORTCUTS.filter(
        (one) =>
          one.said(mac) !== "" &&
          (one.kind === "path" || one.chords[mac ? "mac" : "other"].length > 0),
      );
      expect(ids).toEqual(keyed.map((one) => one.id));
    }
  });

  it("says this keyboard's keys, and where a chat keeps one", () => {
    const mac = sheetRows(true).find((row) => row.id === "region.navigation");
    const other = sheetRows(false).find((row) => row.id === "region.navigation");
    expect(mac).toMatchObject({ keys: "⌘B", where: "" });
    expect(other).toMatchObject({ keys: "Ctrl+B", where: "Not in a chat" });
    expect(sheetRows(true).find((row) => row.id === "palette.open")?.keys).toBe("⌘K · F2");
    expect(sheetRows(false).some((row) => row.id === "app.hide")).toBe(false);
  });

  it("draws no title of its own, the registry's groups as sections and each key in its part", () => {
    render(<ShortcutsView mac />);
    expect(screen.queryByRole("heading")).toBeNull();
    const list = screen.getByRole("grid", { name: "Keyboard shortcuts" });
    for (const group of [
      "Go to",
      "Window",
      "Tabs and chats",
      "Panes",
      "Palette",
      "Dialogs and forms",
    ])
      expect(within(list).getByText(group)).toBeInTheDocument();
    const focus = within(list).getByRole("row", { name: /Focus this pane/ });
    const cap = focus.querySelector('[data-part="shortcut"]');
    expect(cap).toHaveTextContent("⌘⇧↩");
    // Heard as words, not as the symbols drawn.
    expect(cap?.querySelector("[data-heard-as]")).toHaveTextContent("Command Shift Return");
    expect(screen.getByRole("searchbox", { name: "Filter shortcuts" })).toBeInTheDocument();
  });

  it("gives ⌘⇧↩ one act, Focus this pane, which its own words find alone (#4765)", () => {
    for (const mac of [true, false]) {
      const keyed = sheetRows(mac).filter((row) =>
        row.keys.split(" · ").includes(mac ? "⌘⇧↩" : "Ctrl+Shift+Enter"),
      );
      expect(keyed.map((row) => row.id)).toEqual(["focus.toggle"]);
    }
    render(<ShortcutsView mac />);
    const list = screen.getByRole("grid", { name: "Keyboard shortcuts" });
    fireEvent.change(screen.getByRole("searchbox", { name: "Filter shortcuts" }), {
      target: { value: "zoom in a split" },
    });
    const acts = within(list)
      .getAllByRole("row")
      .filter((one) => one.getAttribute("data-part") === "root");
    expect(acts).toHaveLength(1);
    expect(acts[0]).toHaveAccessibleName(/^Focus this pane/);
  });

  it("lists Links in this pane… with the pane's acts, in any pane, with its key on each keyboard (#6358)", () => {
    for (const [mac, keys] of [
      [true, "⇧⌘L"],
      [false, "Ctrl+Shift+L"],
    ] as const) {
      const rows = sheetRows(mac);
      // Not a chat's alone: a shell's pane and a one-job program's list their links too.
      expect(rows.find((row) => row.id === "pane.links")).toMatchObject({
        group: "Panes",
        keys,
        where: "In a pane",
      });
      expect(rows.filter((row) => row.group === "Panes").map((row) => row.id)).toEqual([
        "pane.back",
        "pane.focus",
        "pane.move",
        "pane.links",
      ]);
    }
    render(<ShortcutsView mac />);
    const list = screen.getByRole("grid", { name: "Keyboard shortcuts" });
    const row = within(list).getByRole("row", { name: /^The links on the pane.s screen/ });
    expect(row.querySelector('[data-part="shortcut"]')).toHaveTextContent("⇧⌘L");
  });
});
