// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { BUILT_IN, DEFAULT_THEME, THEME_ALIASES, builtIn } from "./theme";

/**
 * **The built-in themes' ids** (#1838, the #1719 theme-id item). The two themes are
 * `purlis-dark` and `purlis-light`; the ids they had before the rename are read as them, so a
 * project, a workspace or a saved choice that names an old one keeps drawing what it chose.
 *
 * The old ids are spelled in one place in the window, {@link THEME_ALIASES}, and that table is
 * the window's half of the core's name module, which reads the same ids for the same themes.
 * This file never spells one either: it takes them from the table.
 */

/// From `app`, which is vitest's working directory here (`registry.test.ts` reads the core so).
const NAMES = join(process.cwd(), "..", "crates", "purlis-core", "src", "names.rs");

/** `purlis_core::names`' entry `constant`: its purlis spelling, and the old ones it still reads. */
function nameEntry(source: string, constant: string): { write: string; reads: string[] } {
  const at = new RegExp(
    `pub const ${constant}: Name = name\\(\\s*"[^"]*",\\s*Kind::ThemeId,\\s*"([^"]+)",\\s*&\\[([^\\]]*)\\]`,
  ).exec(source);
  expect(at, `${constant} was not found in ${NAMES}`).not.toBeNull();
  const reads = [...(at?.[2] ?? "").matchAll(/"([^"]+)"/g)].map((hit) => hit[1]);
  return { write: at?.[1] ?? "", reads };
}

describe("the built-in themes' ids", () => {
  it("are purlis-dark and purlis-light, and the window comes up in the dark one", () => {
    expect(Object.keys(BUILT_IN).sort()).toEqual(["purlis-dark", "purlis-light"]);
    expect(BUILT_IN["purlis-dark"].appearance).toBe("dark");
    expect(BUILT_IN["purlis-light"].appearance).toBe("light");
    expect(DEFAULT_THEME).toBe(BUILT_IN["purlis-dark"]);
  });

  it("read each old id as the renamed theme: a saved choice loads as the new theme", () => {
    const old = Object.keys(THEME_ALIASES);
    expect(old.length).toBeGreaterThan(0);
    for (const id of old) {
      expect(BUILT_IN[THEME_ALIASES[id]], `${id} names no built-in`).toBeDefined();
      expect(builtIn(id)).toBe(BUILT_IN[THEME_ALIASES[id]]);
      // Its appearance is kept: an old dark id never loads as light.
      expect(builtIn(id)?.appearance).toBe(id.endsWith("light") ? "light" : "dark");
    }
    for (const id of Object.keys(BUILT_IN)) expect(builtIn(id)).toBe(BUILT_IN[id]);
    for (const id of ["system", "solarized/Solarized Dark", "", "toString", "__proto__"])
      expect(builtIn(id), id).toBeUndefined();
  });

  it("are the core's: the alias table is the name module's THEME_DARK and THEME_LIGHT", () => {
    const source = readFileSync(NAMES, "utf8");
    expect(source.length, `${NAMES} read as empty`).toBeGreaterThan(0);
    const fromCore: Record<string, string> = {};
    for (const constant of ["THEME_DARK", "THEME_LIGHT"]) {
      const { write, reads } = nameEntry(source, constant);
      expect(
        BUILT_IN[write],
        `${constant} writes ${write}, which the window does not ship`,
      ).toBeDefined();
      for (const old of reads) fromCore[old] = write;
    }
    expect({ ...THEME_ALIASES }).toEqual(fromCore);
  });
});

/**
 * Every file the window is built from, and its end-to-end specs, as text, keyed by the path from
 * `app` (`literals.test.ts` explains why the glob says which and Node says what).
 */
const SOURCES: Record<string, string> = Object.fromEntries(
  Object.keys({
    ...import.meta.glob("../**/*.css"),
    ...import.meta.glob("../**/*.ts"),
    ...import.meta.glob("../**/*.tsx"),
    ...import.meta.glob("../**/*.json"),
    ...import.meta.glob("../../e2e/**/*.ts"),
  }).map((path) => [
    path.startsWith("../../")
      ? path.slice("../../".length)
      : path.startsWith("../")
        ? `src/${path.slice("../".length)}`
        : `src/theme/${path.slice(2)}`,
    "",
  ]),
);
SOURCES["index.html"] = "";
for (const path of Object.keys(SOURCES)) {
  SOURCES[path] = readFileSync(join(process.cwd(), path), "utf8");
}

/** The one file that may spell an old id, in its alias table and nowhere else in it. */
const ALIAS_TABLE = "src/theme/theme.ts";

describe("no old theme id is spelled in the window", () => {
  it("outside the alias table", () => {
    const old = Object.keys(THEME_ALIASES);
    const paths = Object.keys(SOURCES);
    // A glob that matched nothing would pass by having nothing to fail.
    expect(paths.filter((path) => path.startsWith("src/")).length).toBeGreaterThan(100);
    expect(paths.some((path) => path.startsWith("e2e/"))).toBe(true);
    const spelled = paths
      .filter((path) => path !== ALIAS_TABLE)
      .flatMap((path) =>
        old.filter((id) => SOURCES[path].includes(id)).map((id) => `${path} spells ${id}`),
      );
    expect(spelled).toEqual([]);
  });

  it("and the alias table spells each one once", () => {
    const table = SOURCES[ALIAS_TABLE];
    expect(table.length).toBeGreaterThan(0);
    for (const id of Object.keys(THEME_ALIASES))
      expect(table.split(id).length - 1, `${ALIAS_TABLE} spells ${id}`).toBe(1);
  });
});
