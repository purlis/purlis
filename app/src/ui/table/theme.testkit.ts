// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the table family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // A table's row that cannot be used (#2911).
    '[data-scope="table"][data-part="root"][data-disabled]',
  ],
} satisfies FamilyTheme;
