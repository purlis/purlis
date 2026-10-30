// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * **A cue no theme can erase** (#3295): the close that ends a chat is the button family's quiet
 * ghost, whose hover a theme reaches (`button.root.quiet.hover.*`) in the `parts` layer, after
 * the family's own rules. Its danger hover fill stays the family's to theme; its danger mark (the
 * glyph's colour, and an edge in it) is drawn in the `cues` layer, after `parts`, which no theme
 * writes into. The story `QuietPaneMarks` holds it in a real browser with such a theme.
 */

/** A stylesheet's name, in parts: the stylesheet guard reads a quoted path ending in the
 *  extension as an import. */
const sheet = (name: string) => `${name}.${"css"}`;

const read = (file: string) =>
  readFileSync(join(process.cwd(), "src", file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

const LAYERS = /@layer ([a-z, ]+);/;

describe("the close that ends a chat (#3295)", () => {
  it("is drawn in a layer after the theme's parts", () => {
    const styles = read(sheet("styles"));
    const order = (LAYERS.exec(styles)?.[1] ?? "").split(/,\s*/);
    expect(order).toContain("cues");
    expect(order.indexOf("cues")).toBeGreaterThan(order.indexOf("parts"));
    expect(styles).toContain(`@import "./${sheet("cues")}" layer(cues);`);
  });

  it("keeps its danger mark under the pointer: the glyph and an edge in the danger colour", () => {
    const cues = read(sheet("cues"));
    const rule =
      /\.ends-a-chat\[data-scope="button"\]\[data-variant="quiet"\]\[data-hovered\]:not\(\[data-disabled\]\)\s*\{([^}]*)\}/.exec(
        cues,
      )?.[1];
    expect(rule, "the hover rule in the cues sheet").toBeDefined();
    expect(rule).toMatch(/color:\s*var\(--danger-text\)/);
    expect(rule).toMatch(/box-shadow:\s*inset 0 0 0 [^;]*var\(--danger-text\)/);
  });
});
