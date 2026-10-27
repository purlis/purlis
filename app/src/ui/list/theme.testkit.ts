// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the list family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // A list's row that cannot be used (#2911).
    '[data-scope="list"][data-part="root"][data-disabled]',
    '[data-scope="list"][data-part="root"][data-unavailable]',
  ],
  quiet: [
    // A list's group name (#2911): a day, a file.
    '[data-scope="list"][data-part="header"]',
  ],
} satisfies FamilyTheme;
