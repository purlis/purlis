// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { classicAppearances, handedPurlis } from "./handed.testkit";
import { defaultStoryTheme, storyThemes } from "./storyThemes";
import { BUILT_IN, CLASSIC_THEME, DEFAULT_THEME } from "./theme";

describe("the themes a story is drawn in", () => {
  const purlis = handedPurlis();
  const themes = storyThemes(purlis.appearances, classicAppearances());

  it("draw a story that names none in the theme the window comes up in (#3034)", () => {
    const named = themes[defaultStoryTheme(purlis.default)];
    expect(named).toBeDefined();
    expect(named.theme).toEqual(DEFAULT_THEME);
  });

  it("are each of Purlis's appearances, then each of Purlis Classic's", () => {
    expect(Object.keys(themes)).toEqual([
      "purlis:dark",
      "purlis:light",
      "purlis:dark-high-contrast",
      "purlis-classic:dark",
      "purlis-classic:light",
    ]);
    expect(themes["purlis:light"].theme).toEqual(BUILT_IN["purlis-light"]);
    expect(themes["purlis-classic:dark"].theme).toEqual(CLASSIC_THEME);
    expect(themes["purlis:dark-high-contrast"].title).toBe("Purlis dark high contrast");
  });
});
