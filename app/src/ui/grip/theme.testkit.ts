// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the grip family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  forced: {
    // Its dots keep the system's own colours (#5290).
    drawn: {
      '[data-scope="grip"][data-part="dot"]': { "forced-color-adjust": "none" },
    },
  },
} satisfies FamilyTheme;
