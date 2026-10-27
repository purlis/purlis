// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the select family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // An unavailable select's or combobox's words and marks (#2906).
    '[data-scope="select"][data-part="root"][data-disabled]',
    '[data-scope="combobox"][data-part="root"][data-disabled]',
    '[data-scope="select"][data-part="root"][data-disabled] [data-part="chevron"]',
    '[data-scope="select"][data-part="root"][data-disabled] [data-part="placeholder"]',
    '[data-scope="combobox"][data-part="root"][data-disabled] [data-part="chevron"]',
  ],
  quiet: [
    // A select's placeholder, before a value is chosen (#2906).
    '[data-scope="select"][data-part="placeholder"]',
  ],
  forced: {
    refused: [
      ':is([data-scope="select"], [data-scope="combobox"])[data-part="root"][data-invalid]',
    ],
  },
} satisfies FamilyTheme;
