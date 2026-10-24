// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
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
    for (const group of ["Go to", "Window", "Tabs and chats", "Palette", "Dialogs and forms"])
      expect(within(list).getByText(group)).toBeInTheDocument();
    const focus = within(list).getByRole("row", { name: /Focus mode/ });
    const cap = focus.querySelector('[data-part="shortcut"]');
    expect(cap).toHaveTextContent("⌘⇧↩");
    // Heard as words, not as the symbols drawn.
    expect(cap?.querySelector("[data-heard-as]")).toHaveTextContent("Command Shift Return");
    expect(screen.getByRole("searchbox", { name: "Filter shortcuts" })).toBeInTheDocument();
  });
});
