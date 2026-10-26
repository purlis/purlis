// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **The control primitives** (`primitives/controls.css`, ADR 0094 §4, #1834) are empty now: the
 * button (#2904), the field (#2905), the select and combobox (#2906) and the choices (#2907) are
 * each a component family drawn from its wrapper's attributes (ADR 0104). What is left of the
 * stylesheet is its place in the `components` layer and the notes on where each control went;
 * this holds it there, and holds that no control's rule comes back into it.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

const SRC = join(process.cwd(), "src");
const sheet = (file: string) =>
  readFileSync(join(SRC, file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const CSS = sheet("primitives/controls.css");

/** Every rule of the primitives, as `[selector, body]`. */
const RULES: [string, string][] = [...CSS.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(
  ([, selector, body]) => [selector.trim().replace(/\s+/g, " "), body],
);

describe("the primitives stylesheet", () => {
  it("is in the components layer, above App.css", () => {
    const styles = sheet("styles.css");
    // In two halves: the stylesheet guard reads a quoted path ending in the extension as an
    // import (`literals.test.ts`).
    expect(styles).toContain(
      ['@import "./primitives/controls', 'css" layer(components);'].join("."),
    );
    const order = (/@layer ([\w\s,-]+);/.exec(styles)?.[1] ?? "").split(/\s*,\s*/);
    expect(order.indexOf("components")).toBeGreaterThan(order.indexOf("charter"));
  });

  it("draws no control: each is its family's", () => {
    expect(RULES.map(([selector]) => selector)).toEqual([]);
  });
});
