// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the field family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // An unavailable box's words (#2905).
    '[data-scope="text-input"][data-part="root"][data-disabled]',
    '[data-scope="textarea"][data-part="root"][data-disabled]',
  ],
  forced: {
    refused: [
      ':is([data-scope="text-input"], [data-scope="textarea"])[data-part="root"][data-invalid]',
    ],
  },
} satisfies FamilyTheme;
