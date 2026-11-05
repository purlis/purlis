// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the pane preview says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387, #5291). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  // A part with words on it holds them on its own ground; the arriving pane is the terminal's.
  groundParts: { "pane-preview": [] },
  forced: { drawn: {} },
} satisfies FamilyTheme;
