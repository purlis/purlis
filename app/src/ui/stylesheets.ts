// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every family's stylesheet reaches the window without a line of `styles.css` of its own**
 * (operator ruling 2026-10-26: two family pull requests never touch the same lines). A family's
 * stylesheet is `src/ui/<family>/<family>.css`. `styles.css` imports the families that need a
 * place of their own in the `components` layer by name, and marks where the rest go with
 * {@link FAMILIES}; `vite.config.ts` puts there, as the window, the stories and their tests are
 * built, an import of each family stylesheet `styles.css` does not name, in the families' names'
 * order. So a pull request that adds a family adds its folder and nothing else here.
 */

/** The line of `styles.css` that the families' imports take the place of. */
export const FAMILIES = "/* @families: every other src/ui/<family>/<family>.css, by name */";

/** A family's stylesheet, by the family: `./ui/<family>/<family>.css`, as `styles.css` names it. */
export function familySheet(family: string): string {
  return `./ui/${family}/${family}.css`;
}

/**
 * `styles` with {@link FAMILIES} turned into an import, into the `components` layer, of each of
 * `families` whose stylesheet it does not import already, by name. `styles` without the marker is
 * a mistake, and throws: every family would be left out without a word.
 */
export function withEveryFamily(styles: string, families: readonly string[]): string {
  const lines = styles.split("\n");
  const at = lines.indexOf(FAMILIES);
  if (at < 0) throw new Error(`styles.css has no line ${FAMILIES}`);
  const named = new Set(
    [...styles.matchAll(/^@import "([^"]+)"/gm)].map((hit) => hit[1]).filter(Boolean),
  );
  const imports = [...families]
    .sort()
    .map(familySheet)
    .filter((sheet) => !named.has(sheet))
    .map((sheet) => `@import "${sheet}" layer(components);`);
  return [...lines.slice(0, at), ...imports, ...lines.slice(at + 1)].join("\n");
}
