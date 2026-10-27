// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the tree family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // A tree's row that cannot be used, or run (#2911).
    '[data-scope="tree"][data-part="root"][data-disabled]',
    '[data-scope="tree"][data-part="root"][data-unavailable]',
  ],
  forced: {
    selected: ['[data-scope="tree"][data-part="root"][data-selected]'],
    // A chosen row keeps its edge (#2911).
    drawn: {
      '[data-scope="tree"][data-part="root"][data-selected]': {
        "box-shadow": "inset 2px 0 0 SelectedItemText",
      },
    },
  },
} satisfies FamilyTheme;
