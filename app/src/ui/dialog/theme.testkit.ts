// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the dialog family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  // Of the family's parts (#2910), only its root is a ground a button stands on: the scrim is
  // drawn under the root, and the footer's rule holds nothing.
  groundParts: { dialog: ["root"] },
} satisfies FamilyTheme;
