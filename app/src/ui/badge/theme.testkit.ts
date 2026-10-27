// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the badge family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  forced: { marked: ['[data-scope="count"][data-part="root"][data-variant="hot"]'] },
} satisfies FamilyTheme;
