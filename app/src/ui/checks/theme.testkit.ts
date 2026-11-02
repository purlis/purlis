// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the checks family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387, #4481). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  forced: {
    drawn: {
      '[data-scope="checks"][data-part="item"][data-variant="missing"] [data-part="title"]': {
        "font-weight": "var(--font-weight-strong)",
      },
    },
  },
} satisfies FamilyTheme;
