// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The matrix every story runs across** (#2945, `docs/component-stories.md`): each appearance
 * of each built-in theme (Purlis Classic's dark and light; Purlis's dark, light and dark high
 * contrast), in each density. One cell is one Vitest project (`vitest.project.ts`), so every
 * story runs, is held to axe and plays its keyboard path in every cell, and the decorator in
 * `preview.tsx` draws it in that cell's theme, appearance and density.
 *
 * A picture is taken only in the cells `pictured.json` names, each drawn as its own baseline,
 * named `<story id>_<theme>_<appearance>-<browser>-linux.png`: the theme and the appearance read
 * off the file's name.
 *
 * Plain data and arithmetic, so the Node side (the projects) and the browser side (the
 * decorator) read one definition.
 */

/** The densities a person can choose (`theme.ts`'s `Density`), the first the default. */
export const DENSITIES = ["comfortable", "compact"] as const;

export type CellDensity = (typeof DENSITIES)[number];

/** One cell: a built-in theme by its id, one of its appearances by its id, and a density. */
export type Cell = {
  theme: string;
  appearance: string;
  density: CellDensity;
  /** What the cell is called: its project's name and the end of its pictures' names. */
  name: string;
  /** Whether a picture of each story is taken in it. */
  pictured: boolean;
};

/** The built-in themes as `built-in-themes.json` keeps them: theme id, appearance id, theme. */
export type BuiltInThemes = Readonly<Record<string, Readonly<Record<string, unknown>>>>;

/**
 * A cell's name: `<theme>_<appearance>`, with `_<density>` after it for every density but the
 * default. An underscore, because a story id never has one (Storybook makes each a dash) and
 * Vitest keeps it in a picture's name, where it drops a dot.
 */
export function cellName(theme: string, appearance: string, density: CellDensity): string {
  const named = `${theme}_${appearance}`;
  return density === DENSITIES[0] ? named : `${named}_${density}`;
}

/** Every cell of `themes`, in their order, with a picture taken in those `pictured` names.
 *  Throws on a pictured name that is no cell, so a typo cannot quietly take no pictures. */
export function cellsOf(themes: BuiltInThemes, pictured: readonly string[]): Cell[] {
  const cells = Object.entries(themes).flatMap(([theme, appearances]) =>
    Object.keys(appearances).flatMap((appearance) =>
      DENSITIES.map((density) => {
        const name = cellName(theme, appearance, density);
        return { theme, appearance, density, name, pictured: pictured.includes(name) };
      }),
    ),
  );
  const unknown = pictured.filter((name) => !cells.some((cell) => cell.name === name));
  if (unknown.length > 0)
    throw new Error(`pictured.json names cells the matrix does not have: ${unknown.join(", ")}`);
  return cells;
}

/** The global a story's test page finds its cell on (`cell.setup.ts` puts it there). */
export const CELL_GLOBAL = "__PURLIS_STORY_CELL__";

/** The cell this page's test runs in, or `undefined` outside a test (Storybook's own UI). */
export function cellInTest(): Cell | undefined {
  return (globalThis as Record<string, unknown>)[CELL_GLOBAL] as Cell | undefined;
}

/** A story's picture name in `cell`: its id, then the cell's name. */
export function pictureOf(storyId: string, cell: Cell): string {
  return `${storyId}_${cell.name}`;
}
