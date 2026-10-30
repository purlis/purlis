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
 *
 * A family's forced-colours sheet, `src/ui/<family>/forced-colors.css` (#3387), is put the same
 * way in the place of {@link FORCED}, into the `forced` layer, before the window's own
 * (`theme/forced-colors.css`).
 *
 * Whatever reads `styles.css` for the sheets it imports reads it through {@link builtStyles}, as
 * the build does, or a family that came in through the marker would escape it.
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
/** The line of `styles.css` that the families' imports take the place of. */
export const FAMILIES = "/* @families: every other src/ui/<family>/<family>.css, by name */";
/** The line of `styles.css` that the families' forced-colours sheets take the place of. */
export const FORCED = "/* @forced: every src/ui/<family>/forced-colors.css, by name */";
/**
 * Whether `path` is a forced-colours sheet, the window's (`theme/forced-colors.css`) or a
 * family's: it draws in the system's colours alone, which `forced-colors.test.ts` holds, so the
 * tests that hold every other sheet to the theme's tokens leave it out, as they do `theme/`.
 */
export function isForcedColoursSheet(path) {
    return /(?:^|\/)forced-colors\.css$/.test(path);
}
/** A family's forced-colours sheet, as `styles.css` would name it. */
export function forcedSheet(family) {
    return `./ui/${family}/forced-colors.css`;
}
/**
 * `styles` with {@link FORCED} turned into an import, into the `forced` layer, of each of
 * `families`' forced-colours sheets, by name. `styles` without the marker throws, as
 * {@link withEveryFamily} does.
 */
export function withEveryForcedSheet(styles, families) {
    const lines = styles.split("\n");
    const at = lines.indexOf(FORCED);
    if (at < 0)
        throw new Error(`styles.css has no line ${FORCED}`);
    const imports = [...families].sort().map((one) => `@import "${forcedSheet(one)}" layer(forced);`);
    return [...lines.slice(0, at), ...imports, ...lines.slice(at + 1)].join("\n");
}
/** Every family with a forced-colours sheet of its own under `ui`, the folder `src/ui`. */
export function forcedFamiliesIn(ui) {
    return readdirSync(ui, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => entry.name)
        .filter((name) => existsSync(join(ui, name, "forced-colors.css")));
}
/** `styles` with both markers expanded from `ui`, the folder `src/ui`, as the build does. */
export function withTheFamilies(styles, ui) {
    return withEveryForcedSheet(withEveryFamily(styles, familiesIn(ui)), forcedFamiliesIn(ui));
}
/** A family's stylesheet, by the family: `./ui/<family>/<family>.css`, as `styles.css` names it. */
export function familySheet(family) {
    return `./ui/${family}/${family}.css`;
}
/**
 * `styles` with {@link FAMILIES} turned into an import, into the `components` layer, of each of
 * `families` whose stylesheet it does not import already, by name. `styles` without the marker is
 * a mistake, and throws: every family would be left out without a word.
 */
export function withEveryFamily(styles, families) {
    const lines = styles.split("\n");
    const at = lines.indexOf(FAMILIES);
    if (at < 0)
        throw new Error(`styles.css has no line ${FAMILIES}`);
    const named = new Set([...styles.matchAll(/^@import "([^"]+)"/gm)].map((hit) => hit[1]).filter(Boolean));
    const imports = [...families]
        .sort()
        .map(familySheet)
        .filter((sheet) => !named.has(sheet))
        .map((sheet) => `@import "${sheet}" layer(components);`);
    return [...lines.slice(0, at), ...imports, ...lines.slice(at + 1)].join("\n");
}
/** Every family with a stylesheet of its own under `ui`, the folder `src/ui`. */
export function familiesIn(ui) {
    return readdirSync(ui, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => entry.name)
        .filter((name) => existsSync(join(ui, name, `${name}.css`)));
}
/** **`styles.css` as the window is built from it**: read from `src`, the app's `src` folder, with
 *  every family's import in its marker's place. */
export function builtStyles(src) {
    return withTheFamilies(readFileSync(join(src, "styles.css"), "utf8"), join(src, "ui"));
}
