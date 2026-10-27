// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the activity family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  forced: {
    marked: ['[data-scope="activity"][data-part="count"][data-tone="needs-you"]'],
    drawn: {
      '[data-scope="activity"][data-part="item"][data-selected] > [data-part="indicator"]': {
        "background-color": "SelectedItemText",
      },
    },
  },
} satisfies FamilyTheme;
