/** The line of `styles.css` that the families' imports take the place of. */
export declare const FAMILIES = "/* @families: every other src/ui/<family>/<family>.css, by name */";
/** The line of `styles.css` that the families' forced-colours sheets take the place of. */
export declare const FORCED = "/* @forced: every src/ui/<family>/forced-colors.css, by name */";
/**
 * Whether `path` is a forced-colours sheet, the window's (`theme/forced-colors.css`) or a
 * family's: it draws in the system's colours alone, which `forced-colors.test.ts` holds, so the
 * tests that hold every other sheet to the theme's tokens leave it out, as they do `theme/`.
 */
export declare function isForcedColoursSheet(path: string): boolean;
/** A family's forced-colours sheet, as `styles.css` would name it. */
export declare function forcedSheet(family: string): string;
/**
 * `styles` with {@link FORCED} turned into an import, into the `forced` layer, of each of
 * `families`' forced-colours sheets, by name. `styles` without the marker throws, as
 * {@link withEveryFamily} does.
 */
export declare function withEveryForcedSheet(styles: string, families: readonly string[]): string;
/** Every family with a forced-colours sheet of its own under `ui`, the folder `src/ui`. */
export declare function forcedFamiliesIn(ui: string): string[];
/** `styles` with both markers expanded from `ui`, the folder `src/ui`, as the build does. */
export declare function withTheFamilies(styles: string, ui: string): string;
/** A family's stylesheet, by the family: `./ui/<family>/<family>.css`, as `styles.css` names it. */
export declare function familySheet(family: string): string;
/**
 * `styles` with {@link FAMILIES} turned into an import, into the `components` layer, of each of
 * `families` whose stylesheet it does not import already, by name. `styles` without the marker is
 * a mistake, and throws: every family would be left out without a word.
 */
export declare function withEveryFamily(styles: string, families: readonly string[]): string;
/** Every family with a stylesheet of its own under `ui`, the folder `src/ui`. */
export declare function familiesIn(ui: string): string[];
/** **`styles.css` as the window is built from it**: read from `src`, the app's `src` folder, with
 *  every family's import in its marker's place. */
export declare function builtStyles(src: string): string;
