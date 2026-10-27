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

/** What one family says of its own stylesheet to `tiers.test.ts` and `grounds.test.ts`. */
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
}

const found = import.meta.glob<FamilyTheme>("../ui/*/theme.testkit.ts", {
  eager: true,
  import: "default",
});

/** Every family's {@link FamilyTheme}, in the families' names' order. */
export const FAMILY_THEMES: readonly FamilyTheme[] = Object.keys(found)
  .sort()
  .map((path) => found[path]);
