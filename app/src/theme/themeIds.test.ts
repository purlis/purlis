// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { BUILT_IN_ICONS, DEFAULT_ICONS, ICON_THEME_ALIASES, builtInIcons } from "./icons";
import { BUILT_IN, CLASSIC_THEME, DEFAULT_THEME, THEME_ALIASES } from "./theme";

/**
 * **The built-in themes' ids** (#1838, the #1719 theme-id item). The two themes are
 * `purlis-dark` and `purlis-light`, and the icon theme is `purlis-icons` (#1854); the ids they
 * had before the rename are read as them, so a project, a workspace or a saved choice that
 * names an old one keeps drawing what it chose.
 *
 * The old ids are spelled in two places in the window, {@link THEME_ALIASES} and
 * {@link ICON_THEME_ALIASES}, and each table is the window's half of the core's name module,
 * which reads the same ids for the same themes. This file never spells one either: it takes them
 * from the tables.
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
    // Purlis Classic's dark by its pick id; the window comes up in Purlis's (#3034).
    expect(CLASSIC_THEME).toBe(BUILT_IN["purlis-dark"]);
    expect(DEFAULT_THEME.name).toBe("Purlis");
  });

  it("name a built-in for each old id: which theme an old id draws is the core's", () => {
    // `purlis_core::theme::built_in` reads each old id as the renamed theme, keeping its
    // appearance; the window spells the old ids only to label a pick that names one.
    const old = Object.keys(THEME_ALIASES);
    expect(old.length).toBeGreaterThan(0);
    for (const id of old) {
      expect(BUILT_IN[THEME_ALIASES[id]], `${id} names no built-in`).toBeDefined();
      expect(BUILT_IN[THEME_ALIASES[id]].appearance).toBe(id.endsWith("light") ? "light" : "dark");
    }
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

describe("the built-in icon theme's id (#1854)", () => {
  it("is purlis-icons, and it is what a project that picks none draws", () => {
    expect(Object.keys(BUILT_IN_ICONS)).toEqual(["purlis-icons"]);
    expect(BUILT_IN_ICONS["purlis-icons"]).toBe(DEFAULT_ICONS);
    expect(DEFAULT_ICONS.name).toBe("purlis-icons");
  });

  it("reads each old id as the renamed icon theme: a saved pick loads as purlis-icons", () => {
    const old = Object.keys(ICON_THEME_ALIASES);
    expect(old.length).toBeGreaterThan(0);
    for (const id of old) {
      expect(BUILT_IN_ICONS[ICON_THEME_ALIASES[id]], `${id} names no built-in`).toBeDefined();
      expect(builtInIcons(id)).toBe(BUILT_IN_ICONS[ICON_THEME_ALIASES[id]]);
      // An old icon theme id is no colour theme.
      expect(BUILT_IN[id]).toBeUndefined();
    }
    for (const id of Object.keys(BUILT_IN_ICONS)) expect(builtInIcons(id)).toBe(BUILT_IN_ICONS[id]);
    for (const id of ["purlis-dark", "seti/Seti", "", "toString", "__proto__"])
      expect(builtInIcons(id), id).toBeUndefined();
  });

  it("is the core's: the alias table is the name module's ICON_THEME", () => {
    const source = readFileSync(NAMES, "utf8");
    const { write, reads } = nameEntry(source, "ICON_THEME");
    expect(
      BUILT_IN_ICONS[write],
      `ICON_THEME writes ${write}, which the window does not ship`,
    ).toBeDefined();
    expect({ ...ICON_THEME_ALIASES }).toEqual(Object.fromEntries(reads.map((old) => [old, write])));
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

/** The files that may spell an old id, each in its alias table and nowhere else in it. */
const ALIAS_TABLES: Record<string, readonly string[]> = {
  "src/theme/theme.ts": Object.keys(THEME_ALIASES),
  "src/theme/icons.ts": Object.keys(ICON_THEME_ALIASES),
};

describe("no old theme id is spelled in the window", () => {
  it("outside the alias tables", () => {
    const old = Object.values(ALIAS_TABLES).flat();
    const paths = Object.keys(SOURCES);
    // A glob that matched nothing would pass by having nothing to fail.
    expect(paths.filter((path) => path.startsWith("src/")).length).toBeGreaterThan(100);
    expect(paths.some((path) => path.startsWith("e2e/"))).toBe(true);
    const spelled = paths.flatMap((path) =>
      old
        .filter((id) => !(ALIAS_TABLES[path] ?? []).includes(id))
        .filter((id) => SOURCES[path].includes(id))
        .map((id) => `${path} spells ${id}`),
    );
    expect(spelled).toEqual([]);
  });

  it("and each alias table spells each of its own once", () => {
    for (const [path, ids] of Object.entries(ALIAS_TABLES)) {
      const table = SOURCES[path];
      expect(table.length, path).toBeGreaterThan(0);
      for (const id of ids) expect(table.split(id).length - 1, `${path} spells ${id}`).toBe(1);
    }
  });
});
