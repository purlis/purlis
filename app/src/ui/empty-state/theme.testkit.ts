// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the empty-state family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // An empty state's mark, above its words (#2913).
    '[data-scope="empty-state"][data-part="icon"]',
  ],
} satisfies FamilyTheme;
