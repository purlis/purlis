// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the button family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // A disabled button's words (#2897's palette rules, #2904).
    '[data-scope="button"][data-part="root"][data-disabled]',
  ],
  // Nor is the family's own stylesheet, on its wrapper's attributes, a ground: nothing a button
  // holds is another button.
  groundParts: { button: [] },
  forced: { dangerous: ['[data-scope="button"][data-part="root"][data-variant="danger"]'] },
} satisfies FamilyTheme;
