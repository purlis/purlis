// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every family's stylesheet is in the window's `components` layer once** (`stylesheets.ts`):
 * named in `styles.css`, or put in its `@families` line's place by `vite.config.ts`, by name.
 */
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  builtStyles,
  FAMILIES,
  familiesIn,
  familySheet,
  FORCED,
  forcedFamiliesIn,
  forcedSheet,
  withEveryFamily,
  withEveryForcedSheet,
} from "./stylesheets";

const SRC = join(process.cwd(), "src");

const families = familiesIn(join(SRC, "ui"));

describe("the families' stylesheets", () => {
  it("are each imported into the components layer once, as the window is built", () => {
    const built = builtStyles(SRC);
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
    expect(() => withEveryForcedSheet("/* no marker */", ["a"])).toThrow(/@forced/);
  });
});

describe("the families' forced-colours sheets (#3387)", () => {
  it("are each imported into the forced layer once, before the window's own", () => {
    const built = builtStyles(SRC);
    const forced = forcedFamiliesIn(join(SRC, "ui"));
    expect(forced.length).toBeGreaterThan(0);
    for (const family of forced) {
      const line = `@import "${forcedSheet(family)}" layer(forced);`;
      expect(
        built.split("\n").filter((one) => one.includes(forcedSheet(family))),
        family,
      ).toEqual([line]);
      expect(built.indexOf(line), family).toBeLessThan(built.indexOf("./theme/forced-colors"));
    }
  });

  it("put them in the marker's place, by name", () => {
    const line = (family: string) => `@import "${forcedSheet(family)}" layer(forced);`;
    expect(withEveryForcedSheet(["x", FORCED, "y"].join("\n"), ["b", "a"])).toBe(
      ["x", line("a"), line("b"), "y"].join("\n"),
    );
  });
});
