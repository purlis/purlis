// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { cellName, cellsOf, pictureOf, type BuiltInThemes } from "./matrix";

const json = (file: string): unknown =>
  JSON.parse(readFileSync(fileURLToPath(new URL(file, import.meta.url)), "utf8"));

type Drawn = { values: Record<string, string> };
const THEMES = json("built-in-themes.json") as Record<string, Record<string, Drawn>>;
const PICTURED = json("pictured.json") as string[];

describe("the story matrix (#2945)", () => {
  it("has every appearance of Purlis Classic and Purlis, in each density", () => {
    const names = cellsOf(THEMES as BuiltInThemes, PICTURED).map((cell) => cell.name);
    expect(names).toEqual([
      "purlis_dark",
      "purlis_dark_compact",
      "purlis_dark-high-contrast",
      "purlis_dark-high-contrast_compact",
      "purlis_light",
      "purlis_light_compact",
      "purlis-classic_dark",
      "purlis-classic_dark_compact",
      "purlis-classic_light",
      "purlis-classic_light_compact",
    ]);
  });

  it("pictures both themes' dark and light, and Purlis's high contrast", () => {
    const pictured = cellsOf(THEMES as BuiltInThemes, PICTURED)
      .filter((cell) => cell.pictured)
      .map((cell) => cell.name);
    expect(pictured).toEqual([
      "purlis_dark",
      "purlis_dark-high-contrast",
      "purlis_light",
      "purlis-classic_dark",
      "purlis-classic_light",
    ]);
  });

  it("refuses a pictured cell the matrix does not have", () => {
    expect(() => cellsOf(THEMES as BuiltInThemes, ["purlis_dusk"])).toThrow(/purlis_dusk/);
  });

  it("names a picture by its story, theme and appearance, as Vitest keeps it", () => {
    const [cell] = cellsOf({ purlis: { light: {} } }, ["purlis_light"]);
    expect(pictureOf("ui-button--primary", cell)).toBe("ui-button--primary_purlis_light");
    expect(cellName("purlis-classic", "dark", "compact")).toBe("purlis-classic_dark_compact");
    // Vitest keeps a word character and a dash in a picture's name; a dot it drops.
    expect(pictureOf("ui-button--primary", cell)).toMatch(/^[\w-]+$/);
  });

  it("draws Purlis in its own accent, not Classic's", () => {
    for (const appearance of ["dark", "light"])
      expect(THEMES.purlis[appearance].values["accent.base"]).not.toBe(
        THEMES["purlis-classic"][appearance].values["accent.base"],
      );
  });
});
