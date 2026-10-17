// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { DEFAULT_THEME, apply, drawnFont, type Person, type Theme } from "./theme";

/**
 * **The window draws the person's layer over the theme** (#2326). What the layer holds, and
 * that a font the person names beats the theme's in every theme, is the core's theme engine's
 * (`purlis_core::theme::Person`, `Layer::drawn_font`, `Layer::drawn_geometry`); what is held here
 * is only that the window writes what it was handed, in that order.
 */

/** A theme as the engine hands one with its own geometry and fonts. */
const withFonts: Theme = {
  ...DEFAULT_THEME,
  name: "Dense",
  geometry: {
    comfortable: { "--space-4": "calc(10rem / 14)", "--radius-md": "6px" },
    compact: { "--space-4": "calc(5rem / 14)", "--radius-md": "6px" },
  },
  fonts: {
    ui: { family: "Plex", stack: '"purlis-ui-0123456789ab", sans-serif', faces: [] },
    mono: { family: null, stack: "monospace", faces: [] },
    terminal: { family: "Mono", stack: '"purlis-terminal-0123456789ab", monospace', faces: [] },
  },
};

const comfortable: Person = { density: "comfortable", fonts: {} };

function drawn(theme: Theme, person: Person): CSSStyleDeclaration {
  const root = document.createElement("div");
  apply(theme, root, false, person);
  return root.style;
}

describe("the person's layer", () => {
  it("picks which of the theme's geometries is written by the person's density", () => {
    expect(drawn(withFonts, comfortable).getPropertyValue("--space-4")).toBe("calc(10rem / 14)");
    const compact = drawn(withFonts, { density: "compact", fonts: {} });
    expect(compact.getPropertyValue("--space-4")).toBe("calc(5rem / 14)");
    expect(compact.getPropertyValue("--radius-md")).toBe("6px");
  });

  it("writes the theme's fonts, and the person's in place of them", () => {
    expect(drawn(withFonts, comfortable).getPropertyValue("--font-ui")).toBe(
      '"purlis-ui-0123456789ab", sans-serif',
    );
    const mine: Person = { density: "comfortable", fonts: { ui: '"Atkinson", sans-serif' } };
    for (const theme of [withFonts, DEFAULT_THEME]) {
      expect(drawn(theme, mine).getPropertyValue("--font-ui")).toBe('"Atkinson", sans-serif');
    }
    expect(drawnFont("terminal", withFonts, mine)).toBe(
      '"purlis-terminal-0123456789ab", monospace',
    );
    expect(
      drawnFont("terminal", withFonts, { density: "comfortable", fonts: { terminal: '"X", monospace' } }),
    ).toBe('"X", monospace');
  });
});
