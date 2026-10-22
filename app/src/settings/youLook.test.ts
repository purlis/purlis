// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { DEFAULT_THEME } from "../theme/theme";
import { colourLines, coloursOf } from "./youLook";

/** Two colours to write, read from the default theme so no colour is spelled out here. */
const accent = DEFAULT_THEME.values["accent.base"];
const muted = DEFAULT_THEME.values["text.muted"];

/**
 * **The person's colours as a box of lines** (#2925): one `token #colour` a line. Which token and
 * which colour may be drawn is the engine's to judge (`purlis_core::theme::Person`); the box only
 * reads the lines and writes them back in one order.
 */
describe("the person's colours in Settings", () => {
  it("shows each colour on a line of its own, in token order", () => {
    expect(colourLines({ "text.muted": muted, "accent.base": accent })).toBe(
      `accent.base ${accent}\ntext.muted ${muted}`,
    );
    expect(colourLines(undefined)).toBe("");
  });

  it("reads each line as a token and a colour, and a line with less as nothing", () => {
    expect(coloursOf(`  accent.base   ${accent} \n\ntext.muted\n surface.base ${muted}`)).toEqual({
      "accent.base": accent,
      "surface.base": muted,
    });
  });

  it("gives back what it shows", () => {
    const colours = { "accent.base": accent, "danger.base": muted };
    expect(coloursOf(colourLines(colours))).toEqual(colours);
  });
});

describe("a complaint about the person's settings links to its group (#2925)", () => {
  it("names Your colours, Motion and contrast, or Density and fonts", async () => {
    const { settingsFor } = await import("../appearance");
    expect(settingsFor(['your colour for accent.base, "red", is not #rgb'])).toBe("you.colours");
    expect(settingsFor(["brand.hue is not a colour token of the theme vocabulary"])).toBe(
      "you.colours",
    );
    expect(settingsFor(['motion is "off", which is …'])).toBe("you.motion");
    expect(settingsFor(["contrast is 7, which is …"])).toBe("you.motion");
    expect(settingsFor(['density is "cosy", which is …'])).toBe("you.look");
  });
});
