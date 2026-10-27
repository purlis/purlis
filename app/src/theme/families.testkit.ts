// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * Test-only: **what each family says of its own rules to the theme's tests** (#3387, operator
 * ruling 2026-10-26: two family pull requests never touch the same lines). Each family that has
 * something to say keeps it in its own folder, `src/ui/<family>/theme.testkit.ts`, whose default
 * export is a {@link FamilyTheme}; the tests read every family's, in the families' names' order,
 * after the window's own lists.
 */
import type { Token } from "./theme";

/** What one family's forced-colours sheet (`ui/<family>/forced-colors.css`) draws, as
 *  `forced-colors.test.ts` holds it: each kind's selectors, held to what the window draws for that
 *  kind, and any declaration of its own, exactly. */
export interface ForcedTheme {
  /** What is selected or current: the system's pair, `SelectedItem` and `SelectedItemText`. */
  readonly selected?: readonly string[];
  /** What is selected, underlined too, so it is told without the colour. */
  readonly underlined?: readonly string[];
  /** What needs you: `Mark` and `MarkText`. */
  readonly marked?: readonly string[];
  /** What ends or destroys, or says something is wrong: a dashed outline inside its edge. */
  readonly dangerous?: readonly string[];
  /** A refused box: its own edge dashed, nothing moving. */
  readonly refused?: readonly string[];
  /** What is opened over the window: outlined. */
  readonly opened?: readonly string[];
  /** Any other declaration it draws, by selector and property, exactly. */
  readonly drawn?: Readonly<Record<string, Readonly<Record<string, string>>>>;
}

/** What one family says of its own stylesheets to `tiers.test.ts`, `grounds.test.ts` and
 *  `forced-colors.test.ts`. */
export interface FamilyTheme {
  /** Its rules that may draw in faint: each a mark that is not words, or disabled words. */
  readonly faint?: readonly string[];
  /** Its informational words, each drawn in quiet. */
  readonly quiet?: readonly string[];
  /** The grounds it draws on an element whose attributes a helper of its own spreads, by the
   *  class the element carries (the reader ties a rule to attributes written out). */
  readonly spreadGrounds?: readonly (readonly [string, readonly Token[]])[];
  /** Its background rules the grounds reader cannot tie to an element, each with why no button
   *  stands on what it paints. */
  readonly classless?: readonly string[];
  /** Of its parts' backgrounds (`[data-scope="<scope>"]`), only these parts are grounds a
   *  button stands on, by scope; its other parts' are none. */
  readonly groundParts?: Readonly<Record<string, readonly string[]>>;
  /** What its forced-colours sheet draws. */
  readonly forced?: ForcedTheme;
}

const found = import.meta.glob<FamilyTheme>("../ui/*/theme.testkit.ts", {
  eager: true,
  import: "default",
});

/** Every family's {@link FamilyTheme}, by the family's name. */
export const FAMILY_THEME_BY_NAME: Readonly<Record<string, FamilyTheme>> = Object.fromEntries(
  Object.keys(found)
    .sort()
    .map((path) => [path.split("/").at(-2) ?? path, found[path]]),
);

/** Every family's {@link FamilyTheme}, in the families' names' order. */
export const FAMILY_THEMES: readonly FamilyTheme[] = Object.values(FAMILY_THEME_BY_NAME);
