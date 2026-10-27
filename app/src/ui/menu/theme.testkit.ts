// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the menu family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // A menu's row that cannot run (#2909), and an option of a select's list that cannot be
    // chosen, which is a menu's row (#2906).
    '[data-scope="menu"][data-part="root"][data-disabled]',
    '[data-scope="menu"][data-part="root"][data-unavailable]',
  ],
  forced: {
    selected: [
      '[data-scope="menu"][data-part="root"][data-focused]:not([data-disabled])',
      '[data-scope="menu"][data-part="root"][data-hovered]:not([data-disabled])',
      '[data-scope="menu"][data-part="root"][data-open]',
    ],
    dangerous: ['[data-scope="menu"][data-part="root"][data-variant="danger"]'],
  },
} satisfies FamilyTheme;
