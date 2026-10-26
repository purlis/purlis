// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every family's stylesheet is in the window's `components` layer once** (`stylesheets.ts`):
 * named in `styles.css`, or put in its `@families` line's place by `vite.config.ts`, by name.
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { FAMILIES, familySheet, withEveryFamily } from "./stylesheets";

const SRC = join(process.cwd(), "src");

const families = readdirSync(join(SRC, "ui"), { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .filter((name) => existsSync(join(SRC, "ui", name, `${name}.css`)));

describe("the families' stylesheets", () => {
  it("are each imported into the components layer once, as the window is built", () => {
    const built = withEveryFamily(readFileSync(join(SRC, "styles.css"), "utf8"), families);
    expect(families.length).toBeGreaterThan(0);
    for (const family of families) {
      const imported = built
        .split("\n")
        .filter((line) => line.includes(`"${familySheet(family)}"`));
      expect(imported, family).toEqual([`@import "${familySheet(family)}" layer(components);`]);
    }
  });

  it("put the ones styles.css does not name in the marker's place, by name", () => {
    const line = (family: string) => `@import "${familySheet(family)}" layer(components);`;
    const after = "/* what follows the families */";
    const styles = [line("b"), FAMILIES, after].join("\n");
    expect(withEveryFamily(styles, ["c", "b", "a"])).toBe(
      [line("b"), line("a"), line("c"), after].join("\n"),
    );
  });

  it("refuse a styles.css with no marker, which would leave every new family out", () => {
    expect(() => withEveryFamily("/* no marker */", ["a"])).toThrow(/@families/);
  });
});
